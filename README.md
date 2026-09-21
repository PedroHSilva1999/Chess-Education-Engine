# Chess Education Engine

Microsserviço leve e determinístico para criar atividades educacionais de xadrez. O projeto valida regras, controla sessões e objetivos no backend; a interface apenas apresenta o estado recebido da API.

O frontend estático sobe na Vercel e só escolhe a atividade. O tabuleiro vive no microsserviço da Railway: ele fica parado até receber os parâmetros da sessão e então serve `/play/{id}`.

O objetivo não é competir com o Stockfish. A prioridade é correção, simplicidade, extensibilidade e uma experiência adequada a estudantes.

## Estado do MVP

- Regras oficiais encapsuladas por uma interface própria, usando `shakmaty`.
- FEN, movimentos legais, captura, xeque, mate, afogamento, roque, promoção e en passant.
- Dados para repetição de posição e regra dos cinquenta movimentos.
- Exercícios dirigidos por JSON e objetivos compostáveis por contrato.
- Mate em um, escape do xeque, captura, movimento de peça, promoção, abertura, jogo livre e partida contra engine.
- Engine educacional simples com material, alpha-beta/negamax, profundidade e tempo limitados.
- Sessões isoladas por UUID, métricas e repositório em memória atrás de uma trait.
- API REST e gRPC compartilhando o mesmo `SessionService`.
- Interface web estática, com tabuleiro por clique e drag and drop, desafios e partida contra bot.
- Docker multi-stage, usuário sem privilégios, filesystem somente leitura e healthcheck.
- Testes unitários, de propriedade, PERFT, sessão e API.

O adaptador Redis está deliberadamente fora do MVP. `SessionRepository` é o ponto de extensão para adicioná-lo sem alterar domínio, REST, gRPC ou interface. Com o repositório em memória, sessões não sobrevivem a reinícios e cada réplica tem seu próprio estado. Na Railway, use uma instância.

## Arquitetura

```text
Vercel (web/)                 Railway (chess-api)
catálogo ──POST parâmetros──► REST :$PORT
          ◄── ui_url /play/id ──┘
navegador ──────────────────► GET /play/{id}  (tabuleiro)
                              gRPC :50051 (opcional)
```

```text
crates/
├── chess-core/       Tipos de domínio e regras; não conhece HTTP
├── chess-engine/     Busca leve, limitada por tempo
├── chess-education/  Exercícios, objetivos e geradores data-driven
├── chess-session/    Estado, métricas, repositório e aplicação
└── chess-api/        Adaptadores REST/gRPC, tabuleiro em /play/{id} e binário
web/                  Catálogo estático para a Vercel
crates/chess-web/     HTML/CSS/JS do tabuleiro embutidos no microsserviço
proto/
└── chess.proto
```

Dependências apontam para dentro: transporte depende da aplicação, que depende do domínio. O domínio nunca depende de Axum, Tonic ou do frontend.

## Deploy

### Railway (API)

1. Crie um serviço a partir deste repositório. O `Dockerfile` e o `railway.toml` já estão na raiz.
2. Railway injeta `PORT`; o binário escuta em `0.0.0.0:$PORT`.
3. Variáveis:

| Variável | Uso |
|---|---|
| `FRONTEND_ORIGIN` | Origem da Vercel, por exemplo `https://seu-app.vercel.app`. Várias origens: lista separada por vírgula. Não use `*`. |
| `PUBLIC_API_URL` | URL pública deste microsserviço, usada em `ui_url`. Ex.: `https://chess-api.up.railway.app`. |
| `CORS_ORIGIN` | Alias de `FRONTEND_ORIGIN` se esta não existir. |
| `RUST_LOG` | Opcional. Padrão `info`. |
| `GRPC_ADDR` | Opcional. Padrão `0.0.0.0:50051`. |

4. Healthcheck: `GET /health`.
5. Copie a URL pública, por exemplo `https://chess-api.up.railway.app`.

### Vercel (frontend)

1. Root Directory: `web`.
2. Variável de ambiente de **build**: `CHESS_API_URL=https://sua-api.up.railway.app` (sem barra no final).
3. Depois do primeiro deploy, volte na Railway e confirme `FRONTEND_ORIGIN` com a URL final da Vercel, incluindo `https://`. O catálogo redireciona para `{CHESS_API_URL}/play/{id}`.

## Executar localmente

### Docker

```bash
docker compose up --build
```

Depois:

- Interface (catálogo): `http://localhost:4173`
- REST e tabuleiro: `http://localhost:8080`
- gRPC: `localhost:50051`
- Healthcheck: `http://localhost:8080/health`

Parar:

```bash
docker compose down
```

### Rust + frontend estático

Requisitos: Rust 1.95 ou superior e Node.js para o servidor estático. Em Windows com toolchain MSVC, instale também o Visual C++ Build Tools.

Terminal 1:

```bash
cargo run --release -p chess-api
```

Terminal 2:

```bash
cd web
npm run dev
```

A interface fica em `http://localhost:4173` e chama `http://localhost:8080`.

Variáveis da API:

| Variável | Padrão | Uso |
|---|---:|---|
| `PORT` | `8080` | Porta REST quando `HTTP_ADDR` não existe |
| `HTTP_ADDR` | — | Endereço REST completo; tem prioridade sobre `PORT` |
| `GRPC_ADDR` | `0.0.0.0:50051` | Endereço gRPC |
| `RUST_LOG` | `info` | Filtro de logs estruturados |
| `FRONTEND_ORIGIN` | origens localhost | Origem permitida no CORS; obrigatória em produção |
| `PUBLIC_API_URL` | — | URL pública do microsserviço para `ui_url` |

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

A resposta inclui `session_id`, FEN, objetivo, movimentos legais, configuração de UI e `ui_url`. Abra `{PUBLIC_API_URL}/play/{session_id}` — o tabuleiro é servido pelo microsserviço.

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
| `GET` | `/` | Descrição mínima da API |
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
- CORS restrito a `FRONTEND_ORIGIN` em produção; sem `*`.
- Erros públicos não expõem stack trace.
- Container sem privilégios e filesystem read-only no Compose.
- `x-request-id` criado ou propagado pela camada HTTP.
- Logs JSON via `tracing`/`tracing-subscriber`.
- A URL da API no frontend (`CHESS_API_URL`) não é um segredo; a chave de nenhum provedor de modelo entra no browser.

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
- Frontend e API são artefactos de deploy separados.
- Não há Kafka, Kubernetes, CQRS, banco obrigatório ou múltiplos serviços no MVP.
