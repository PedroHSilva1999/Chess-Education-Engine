(() => {
  "use strict";

  const SESSION_ID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  const challengeGroups = [
    {
      title: "Táticas",
      items: [
        { id: "mate", title: "Mate em 1", copy: "Dama e rei contra rei" },
        { id: "mate-rook", title: "Mate com torre", copy: "Encaixe o rei na borda" },
        { id: "mate-scholar", title: "Mate do pastor", copy: "O clássico Qxf7" },
        { id: "mate-backrank", title: "Mate na última fileira", copy: "A torre fecha o corredor" },
        { id: "fools-mate", title: "Mate do tolo", copy: "Jogue com as pretas" },
        { id: "escape", title: "Escape do xeque", copy: "Proteja o rei" },
        { id: "check", title: "Dê xeque", copy: "Atace o rei rival" },
        { id: "capture", title: "Capture a torre", copy: "A peça está desprotegida" },
        { id: "material", title: "Ganhe material", copy: "Saia com vantagem" },
        { id: "promote", title: "Promova o peão", copy: "Chegue à oitava fileira" },
      ],
    },
    {
      title: "Peças",
      items: [
        { id: "knight", title: "Treino de cavalo", copy: "Salte até c3" },
        { id: "bishop", title: "Treino de bispo", copy: "Ocupe a diagonal até g5" },
        { id: "rook", title: "Treino de torre", copy: "Suba a coluna até a7" },
        { id: "queen", title: "Treino de dama", copy: "Alcance h5 em um lance" },
        { id: "pawn", title: "Treino de peão", copy: "Avance para e4" },
        { id: "opening", title: "Abra com e4", copy: "O primeiro lance da partida" },
      ],
    },
    {
      title: "Partida",
      items: [
        { id: "bot-intermediate", title: "Bot intermediário", copy: "Mesma partida, busca um pouco mais profunda" },
      ],
    },
  ];
  const requests = {
    bot: { mode: "full_game", difficulty: "beginner", opponent: { type: "engine", difficulty: 2 } },
    "bot-intermediate": { mode: "full_game", difficulty: "intermediate", opponent: { type: "engine", difficulty: 3 } },
    mate: { mode: "exercise", exercise: { type: "checkmate", difficulty: "beginner", max_moves: 1 } },
    "mate-rook": {
      mode: "exercise",
      position: { fen: "7k/8/6K1/8/8/8/8/7R w - - 0 1" },
      objective: { type: "checkmate", max_moves: 1 },
      difficulty: "beginner",
    },
    "mate-scholar": {
      mode: "exercise",
      position: { fen: "r1bqkbnr/pppp1ppp/2n5/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 2 3" },
      objective: { type: "checkmate", max_moves: 1 },
      difficulty: "beginner",
    },
    "mate-backrank": {
      mode: "exercise",
      position: { fen: "6k1/5ppp/8/8/8/8/8/4R1K1 w - - 0 1" },
      objective: { type: "checkmate", max_moves: 1 },
      difficulty: "beginner",
    },
    "fools-mate": {
      mode: "exercise",
      player_color: "black",
      position: { fen: "rnbqkbnr/pppp1ppp/8/4p3/6P1/5P2/PPPPP2P/RNBQKBNR b KQkq - 0 2" },
      objective: { type: "checkmate", max_moves: 1 },
      difficulty: "beginner",
    },
    escape: { mode: "exercise", exercise: { type: "check_escape", difficulty: "beginner", max_moves: 1 } },
    check: {
      mode: "exercise",
      position: { fen: "4k3/8/8/8/8/8/4Q3/4K3 w - - 0 1" },
      objective: { type: "check" },
      difficulty: "beginner",
    },
    capture: { mode: "exercise", exercise: { type: "capture_training", difficulty: "beginner", max_moves: 2 } },
    material: {
      mode: "exercise",
      position: { fen: "4k3/8/8/8/4r3/8/8/4QK2 w - - 0 1" },
      objective: { type: "win_material" },
      difficulty: "beginner",
    },
    promote: {
      mode: "exercise",
      position: { fen: "4k3/P7/8/8/8/8/8/4K3 w - - 0 1" },
      objective: { type: "promote" },
      difficulty: "beginner",
    },
    knight: { mode: "piece_training", piece: "knight", difficulty: "beginner", config: { show_legal_moves: true, number_of_tasks: 1 } },
    bishop: { mode: "piece_training", piece: "bishop", difficulty: "beginner", config: { show_legal_moves: true, number_of_tasks: 1, target: "g5" } },
    rook: { mode: "piece_training", piece: "rook", difficulty: "beginner", config: { show_legal_moves: true, number_of_tasks: 1, target: "a7" } },
    queen: { mode: "piece_training", piece: "queen", difficulty: "beginner", config: { show_legal_moves: true, number_of_tasks: 1, target: "h5" } },
    pawn: { mode: "piece_training", piece: "pawn", difficulty: "beginner", config: { show_legal_moves: true, number_of_tasks: 1, target: "e4" } },
    opening: {
      mode: "exercise",
      position: { fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1" },
      objective: { type: "complete_sequence", moves: ["e2e4"] },
      difficulty: "beginner",
    },
  };

  const $ = (selector) => document.querySelector(selector);

  document.addEventListener("DOMContentLoaded", init);

  async function init() {
    renderChallengeGroups();
    document.querySelectorAll("[data-create]").forEach((button) => {
      button.addEventListener("click", () => createActivity(button.dataset.create));
    });
    try {
      const health = await fetch(apiUrl("/health"));
      if (!health.ok) throw new Error("Serviço indisponível");
      $("#connection").textContent = "online";
      $("#connection").classList.add("online");
    } catch (error) {
      $("#connection").textContent = "offline";
      $("#launcher-title").textContent = "Atividades temporariamente indisponíveis";
      $("#launcher-copy").textContent = "Não foi possível conectar ao microsserviço. Tente novamente em alguns instantes.";
      showError(error);
    }
  }

  function renderChallengeGroups() {
    const root = $("#challenge-groups");
    root.replaceChildren();
    challengeGroups.forEach((group) => {
      const section = document.createElement("section");
      section.className = "challenge-group";
      const heading = document.createElement("h3");
      heading.textContent = group.title;
      const grid = document.createElement("div");
      grid.className = "activity-grid";
      group.items.forEach((item) => {
        const button = document.createElement("button");
        button.type = "button";
        button.dataset.create = item.id;
        button.append(item.title, document.createElement("small"));
        button.querySelector("small").textContent = item.copy;
        grid.append(button);
      });
      section.append(heading, grid);
      root.append(section);
    });
  }

  async function createActivity(kind) {
    const request = requests[kind];
    if (!request) {
      showError(new Error("Atividade desconhecida."));
      return;
    }
    try {
      const data = await api("/api/v1/sessions", { method: "POST", body: request });
      const id = typeof data.session_id === "string" ? data.session_id : "";
      if (!SESSION_ID_RE.test(id)) throw new Error("A API devolveu uma sessão inválida.");
      location.href = playUrl(id);
    } catch (error) {
      showError(error);
    }
  }

  function apiBase() {
    const raw = String(window.CHESS_API_URL || "").trim();
    if (!raw) return "";
    try {
      const url = new URL(raw);
      if (url.protocol !== "http:" && url.protocol !== "https:") return "";
      return `${url.origin}${url.pathname}`.replace(/\/$/, "");
    } catch {
      return "";
    }
  }

  function apiUrl(path) {
    return `${apiBase()}${path}`;
  }

  function playUrl(id) {
    return `${apiBase()}/play/${id}`;
  }

  async function api(url, options = {}) {
    const response = await fetch(apiUrl(url), {
      ...options,
      headers: { "Content-Type": "application/json", ...(options.headers || {}) },
      body: options.body ? JSON.stringify(options.body) : undefined,
    });
    const data = response.status === 204 ? null : await response.json();
    if (!response.ok) {
      const error = new Error(data?.error?.message || `Erro HTTP ${response.status}`);
      error.status = response.status;
      error.code = data?.error?.code;
      throw error;
    }
    return data;
  }

  function showError(error) {
    const toast = $("#toast");
    toast.textContent = error.message || "Não foi possível concluir a operação.";
    toast.hidden = false;
    setTimeout(() => { toast.hidden = true; }, 5000);
  }
})();
