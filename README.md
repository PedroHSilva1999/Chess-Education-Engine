# Chess Education Engine

Microsserviço leve e determinístico para criar atividades educacionais de xadrez. O projeto valida regras, controla sessões e objetivos no backend; a interface apenas apresenta o estado recebido da API.

O objetivo não é competir com o Stockfish. A prioridade é correção, simplicidade, extensibilidade e uma experiência adequada a estudantes.

<img width="1259" height="905" alt="image" src="https://github.com/user-attachments/assets/ef168884-1ccc-4413-90b2-55468633cf04" />


## Estado do MVP

- Regras oficiais encapsuladas por uma interface própria, usando `shakmaty`.
- FEN, movimentos legais, captura, xeque, mate, afogamento, roque, promoção e en passant.
- Dados para repetição de posição e regra dos cinquenta movimentos.
- Exercícios dirigidos por JSON e objetivos compostáveis por contrato.
- Mate em um, escape do xeque, captura, movimento de peça, jogo livre e partida contra engine.
- Engine educacional simples com material, alpha-beta/negamax, profundidade e tempo limitados.
- Sessões isoladas por UUID, métricas e repositório em memória atrás de uma trait.
- API REST e gRPC compartilhando o mesmo `SessionService`.
- Tabuleiro web responsivo com clique, drag and drop, movimentos legais, último lance, promoção, dicas e progresso.
- Docker multi-stage, usuário sem privilégios, filesystem somente leitura e healthcheck.
- Testes unitários, de propriedade, PERFT, sessão e API.

O adaptador Redis está deliberadamente fora do MVP. `SessionRepository` é o ponto de extensão para adicioná-lo sem alterar domínio, REST, gRPC ou interface. Com o repositório em memória, sessões não sobrevivem a reinícios e cada réplica tem seu próprio estado.

## Arquitetura

```text
REST :8080 ─┐
            ├── SessionService ── SessionRepository
gRPC :50051 ┘         │                  └── InMemory (MVP)
                      ├── Education Engine
                      │    ├── ExerciseFactory
                      │    ├── PositionGenerator
                      │    └── ExerciseValidator
                      ├── Chess AI Engine
                      └── ChessRulesEngine
                           └── ShakmatyRules
```

```text
crates/
├── chess-core/       Tipos de domínio e regras; não conhece HTTP
├── chess-engine/     Busca leve, limitada por tempo
├── chess-education/  Exercícios, objetivos e geradores data-driven
├── chess-session/    Estado, métricas, repositório e aplicação
├── chess-api/        Adaptadores REST/gRPC e binário
└── chess-web/        Interface estática embutida no binário
proto/
└── chess.proto
```

Dependências apontam para dentro: transporte depende da aplicação, que depende do domínio. O domínio nunca depende de Axum, Tonic ou do frontend.

## Executar com Docker

Requisito: Docker com Compose.

```bash
docker compose up --build
```

Depois:

- Interface e REST: `http://localhost:8080`
- gRPC: `localhost:50051`
- Healthcheck: `http://localhost:8080/health`
- Readiness: `http://localhost:8080/ready`

Parar:

```bash
docker compose down
```

## Executar com Rust

Requisitos: Rust 1.95 ou superior. Em Windows com toolchain MSVC, instale também o Visual C++ Build Tools.

```bash
cargo run --release -p chess-api
```

Variáveis:

| Variável | Padrão | Uso |
|---|---:|---|
| `HTTP_ADDR` | `0.0.0.0:8080` | Endereço REST/web |
| `GRPC_ADDR` | `0.0.0.0:50051` | Endereço gRPC |
| `RUST_LOG` | `info` | Filtro de logs estruturados |
| `CORS_ORIGIN` | `*` | Origem permitida; configure explicitamente em produção |

## REST API

### Criar uma sessão

```bash
curl -X POST http://localhost:8080/api/v1/sessions \
  -H "Content-Type: application/json" \
  -d '{
    "mode": "exercise",
    "exercise": {
      "type": "checkmate",
      "difficulty": "beginner",
      "max_moves": 1
    }
  }'
```

A resposta inclui `session_id`, FEN, objetivo, movimentos legais, configuração de UI e `ui_url`. Abra `http://localhost:8080/play/{session_id}`.

### Jogar

```bash
curl -X POST http://localhost:8080/api/v1/sessions/SEU_UUID/moves \
  -H "Content-Type: application/json" \
  -d '{"from":"g6","to":"g7","promotion":null}'
```

O feedback usa `message_key` e variáveis estruturadas. Tradução e textos longos permanecem fora da engine.

### Demais rotas

| Método | Rota | Função |
|---|---|---|
| `GET` | `/health` | Processo ativo |
| `GET` | `/ready` | Serviço pronto |
| `POST` | `/api/v1/sessions` | Cria sessão |
| `GET` | `/api/v1/sessions/{id}` | Consulta sessão |
| `POST` | `/api/v1/sessions/{id}/moves` | Valida e aplica lance |
| `POST` | `/api/v1/sessions/{id}/hint` | Dica progressiva estruturada |
| `POST` | `/api/v1/sessions/{id}/reset` | Reinicia sessão |
| `DELETE` | `/api/v1/sessions/{id}` | Remove sessão |

## gRPC

O contrato está em `proto/chess.proto`. REST e gRPC chamam a mesma instância de `SessionService`.

Exemplo com `grpcurl`:

```bash
grpcurl -plaintext \
  -import-path proto \
  -proto chess.proto \
  -d '{"mode":"exercise","exercise_type":"checkmate","difficulty":"beginner","max_moves":1}' \
  localhost:50051 \
  chess.education.v1.ChessEducation/CreateSession
```

Para configurações completas, envie o mesmo JSON da API REST em `request_json`.

## Exercícios dirigidos por dados

Um exercício customizado não requer nova rota ou tela:

```json
{
  "mode": "exercise",
  "position": {
    "fen": "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"
  },
  "objective": {
    "type": "move_piece",
    "piece": "pawn",
    "target": "e4"
  },
  "difficulty": "beginner"
}
```

Há exemplos prontos em `examples/`.

Objetivos suportados pelo contrato:

- `checkmate`
- `check`
- `capture`
- `move_piece`
- `reach_square`
- `promote`
- `escape_check`
- `win_material`
- `survive`
- `best_move`
- `complete_sequence`
- `play_full_game`

## Como estender

### Novo Objective

1. Adicione a variante a `ObjectiveSpec` em `chess-education/src/model.rs`.
2. Implemente sua avaliação em `DefaultObjectiveEvaluator` ou crie outro `ExerciseValidator`.
3. Adicione testes com posição inicial, progresso, sucesso e falha.
4. Aponte um JSON para o novo `type`; nenhuma rota nova é necessária.

### Novo PositionGenerator

1. Implemente `PositionGenerator`.
2. Gere ou transforme templates com rotação, espelhamento ou troca de cores.
3. Valide toda posição com `ChessRulesEngine::validate_position`.
4. Injete o gerador na fábrica de exercícios.

### Nova modalidade

Modele-a como composição de posição, objetivo, regras e oponente. Só crie um novo `ExerciseType` quando a modalidade tiver semântica de domínio própria; não crie handlers HTTP específicos.

### Redis

Implemente `SessionRepository` em `chess-session`. Use serialização de `ChessSession`, TTL e compare-and-swap/lock por sessão para evitar perda de atualização. Depois troque somente a composição em `application_service()`.

## Engine educacional

`SimpleEngine` usa busca negamax com poda alpha-beta, avaliação material e limites rígidos. A dificuldade combina:

- profundidade;
- limite de tempo;
- quantidade de alternativas aceitáveis;
- ruído de avaliação preparado no contrato.

A busca roda em `spawn_blocking`, não no executor assíncrono do Tokio, e tem timeout externo. Próximas evoluções naturais são tabelas peça-casa, quiescence, ordenação de movimentos, transposition table e Zobrist hashing — medidas por benchmark, não adicionadas prematuramente.

## Segurança e operação

- Payload REST limitado a 32 KiB.
- FEN e casas validados no domínio.
- UUID v4 para sessões.
- Limite de movimentos por exercício.
- Busca limitada por profundidade, tempo e timeout.
- CORS configurável.
- Erros públicos não expõem stack trace.
- Container sem privilégios e filesystem read-only no Compose.
- `x-request-id` criado ou propagado pela camada HTTP.
- Logs JSON via `tracing`/`tracing-subscriber`.

Rate limiting deve ser aplicado no gateway/reverse proxy ou como uma nova camada Tower, conforme a política da plataforma. A engine não armazena dados pessoais.

## Testes e qualidade

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --locked
```

Sem toolchain nativa completa, use o mesmo ambiente Linux do build:

```bash
docker run --rm -v "$PWD:/app" -w /app rust:1.97-bookworm cargo test --workspace
```

Os testes cobrem, entre outros:

- posição inicial e movimentos legais;
- movimento ilegal;
- roque, en passant e promoção;
- xeque-mate e afogamento;
- PERFT da posição inicial até profundidade 3 (`20`, `400`, `8902`);
- propriedade de que todo movimento legal aplicado produz outra posição válida;
- objetivo de mate em um;
- dicas estruturadas;
- isolamento de sessões;
- fluxo completo pela API REST.

## Benchmarks

```bash
cargo bench -p chess-core
```

O benchmark Criterion mede parsing FEN, geração de movimentos legais e aplicação/validação de movimento. Novas otimizações devem começar com uma medição reproduzível aqui.

## Decisões importantes

- `shakmaty` fica atrás de `ChessRulesEngine`; trocar a biblioteca não altera os consumidores.
- Bitboards nunca aparecem no contrato público.
- O LLM futuro poderá selecionar, explicar e personalizar atividades, mas não validar movimentos nem decidir mate.
- O frontend não conhece regras: recebe FEN, movimentos legais, objetivo e configuração da API.
- Não há Kafka, Kubernetes, CQRS, banco obrigatório ou múltiplos serviços no MVP.
