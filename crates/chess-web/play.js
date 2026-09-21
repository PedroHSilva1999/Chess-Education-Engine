(() => {
  "use strict";

  const SESSION_ID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
  const pieces = {
    P: "♟", N: "♞", B: "♝", R: "♜", Q: "♛", K: "♚",
    p: "♟", n: "♞", b: "♝", r: "♜", q: "♛", k: "♚",
  };
  const objectiveText = {
    checkmate: ["Mate em 1", "Encontre o movimento que deixa o rei adversário sem saída."],
    escape_check: ["Escape do xeque", "Faça um movimento legal que retire seu rei do ataque."],
    capture: ["Treino de captura", "Capture a peça adversária indicada pelo exercício."],
    move_piece: ["Treino de movimento", "Leve a peça até a casa-alvo usando seu movimento correto."],
    play_full_game: ["Jogar com bot", "Jogue uma partida contra uma engine ajustada ao nível educacional."],
    best_move: ["Encontre o melhor lance", "Analise a posição e escolha a continuação mais forte."],
    check: ["Dê xeque", "Atace o rei adversário com um lance legal."],
    promote: ["Promoção", "Avance o peão até a última fileira e escolha a peça."],
    win_material: ["Ganhe material", "Capture uma peça e fique com vantagem material."],
    complete_sequence: ["Abertura", "Jogue o lance pedido para abrir a partida."],
    reach_square: ["Alcance a casa", "Leve uma peça até a casa indicada."],
  };
  const messages = {
    objective_in_progress: "Movimento legal. Continue buscando o objetivo.",
    objective_completed: "Muito bem! Objetivo concluído.",
    illegal_move: "Essa peça não pode fazer esse movimento nesta posição.",
    move_limit_reached: "O limite de movimentos foi alcançado. Reinicie e tente outra ideia.",
    better_move_available: "Esse lance é legal, mas existe uma continuação melhor.",
  };

  const state = { session: null, selected: null, busy: false, hint: null };
  const $ = (selector) => document.querySelector(selector);
  const sessionId = parseSessionId(location.pathname);

  document.addEventListener("DOMContentLoaded", init);

  async function init() {
    bindLauncherLinks();
    $("#hint-button").addEventListener("click", requestHint);
    $("#reset-button").addEventListener("click", resetSession);
    try {
      const health = await fetch("/health");
      if (!health.ok) throw new Error("Serviço indisponível");
      $("#connection").textContent = "online";
      $("#connection").classList.add("online");
      if (!sessionId) {
        showExpired();
        return;
      }
      try {
        await loadSession();
        $("#expired").hidden = true;
        $("#workspace").hidden = false;
      } catch (error) {
        if (error.status === 404) {
          showExpired();
          return;
        }
        throw error;
      }
    } catch (error) {
      $("#connection").textContent = "offline";
      showError(error);
    }
  }

  function bindLauncherLinks() {
    const home = launcherUrl();
    ["#home-link", "#expired-home"].forEach((selector) => {
      const link = $(selector);
      if (home) link.href = home;
    });
  }

  function showExpired() {
    state.session = null;
    $("#workspace").hidden = true;
    $("#expired").hidden = false;
    $("#page-title").textContent = "Sessão expirada";
  }

  async function loadSession() {
    state.session = await api(`/api/v1/sessions/${sessionId}`);
    state.selected = null;
    render();
  }

  function render() {
    const session = state.session;
    if (!session) return;
    const kind = session.exercise.objective.type;
    const [title, copy] = objectiveText[kind] || [session.exercise.title_key, "Conclua o objetivo apresentado pela atividade."];
    $("#page-title").textContent = title;
    $("#objective-title").textContent = title;
    $("#objective-copy").textContent = copy;
    $("#progress-bar").style.width = `${Math.round((session.objective.progress || 0) * 100)}%`;
    renderMetrics(session.metrics);
    renderBoard(session.board.fen, session.legal_moves, session.history, session.ui.orientation);
    renderCaptured(session.exercise.initial_position.fen, session.board.fen);
    const done = ["completed", "failed", "abandoned"].includes(session.status);
    $("#hint-button").disabled = state.busy || done;
    $("#reset-button").disabled = state.busy;
    if (session.status === "completed") setFeedback("objective_completed", "success");
  }

  function renderBoard(fen, legalMoves, history, orientation) {
    const board = $("#board");
    board.replaceChildren();
    const position = parseFen(fen);
    const files = orientation === "black" ? [..."hgfedcba"] : [..."abcdefgh"];
    const ranks = orientation === "black" ? [1,2,3,4,5,6,7,8] : [8,7,6,5,4,3,2,1];
    const selectedTargets = new Set(
      state.selected ? legalMoves.filter((move) => move.from === state.selected).map((move) => move.to) : [],
    );
    const last = history.at(-1)?.chess_move;

    ranks.forEach((rank, row) => files.forEach((file, column) => {
      const squareName = `${file}${rank}`;
      const piece = position[squareName];
      const square = document.createElement("button");
      square.type = "button";
      square.className = `square ${(row + column) % 2 ? "dark" : "light"}`;
      square.dataset.square = squareName;
      square.setAttribute("role", "gridcell");
      square.setAttribute("aria-label", `${squareName}${piece ? `, ${pieceName(piece)}` : ""}`);
      if (piece) {
        square.classList.add("occupied");
        const glyph = document.createElement("span");
        glyph.className = "piece";
        glyph.classList.add(piece === piece.toUpperCase() ? "white-piece" : "black-piece");
        glyph.textContent = pieces[piece];
        glyph.draggable = true;
        glyph.addEventListener("dragstart", (event) => event.dataTransfer.setData("text/plain", squareName));
        square.append(glyph);
      }
      if (squareName === state.selected) square.classList.add("selected");
      if (selectedTargets.has(squareName)) square.classList.add("legal");
      if (last && (squareName === last.from || squareName === last.to)) square.classList.add("last");
      if (column === 0) square.append(coord("rank", String(rank)));
      if (row === 7) square.append(coord("file", file));
      square.addEventListener("click", () => selectSquare(squareName));
      square.addEventListener("dragover", (event) => event.preventDefault());
      square.addEventListener("drop", (event) => {
        event.preventDefault();
        const from = event.dataTransfer.getData("text/plain");
        if (from) sendMove(from, squareName);
      });
      board.append(square);
    }));
  }

  function selectSquare(square) {
    if (state.busy || !state.session) return;
    const legal = state.session.legal_moves;
    if (state.selected) {
      const match = legal.find((move) => move.from === state.selected && move.to === square);
      if (match) return sendMove(state.selected, square, match.promotion);
    }
    if (legal.some((move) => move.from === square)) {
      state.selected = square;
      render();
    } else {
      state.selected = null;
      render();
    }
  }

  async function sendMove(from, to, knownPromotion) {
    const candidates = state.session.legal_moves.filter((move) => move.from === from && move.to === to);
    if (!candidates.length) {
      setFeedback("illegal_move", "error");
      return;
    }
    let promotion = knownPromotion || null;
    if (candidates.some((move) => move.promotion)) {
      promotion = await askPromotion();
    }
    await withBusy(async () => {
      const result = await api(`/api/v1/sessions/${sessionId}/moves`, {
        method: "POST",
        body: { from, to, promotion },
      });
      state.selected = null;
      state.session = await api(`/api/v1/sessions/${sessionId}`);
      setFeedback(result.feedback.message_key, result.valid ? "success" : "error");
      render();
    });
  }

  function askPromotion() {
    const dialog = $("#promotion");
    dialog.hidden = false;
    return new Promise((resolve) => {
      const onClick = (event) => {
        const piece = event.target.dataset.piece;
        if (!piece) return;
        dialog.hidden = true;
        dialog.removeEventListener("click", onClick);
        resolve(["queen", "rook", "bishop", "knight"].includes(piece) ? piece : "queen");
      };
      dialog.addEventListener("click", onClick);
    });
  }

  async function requestHint() {
    await withBusy(async () => {
      const result = await api(`/api/v1/sessions/${sessionId}/hint`, { method: "POST" });
      state.hint = result.hint;
      state.session.metrics = result.metrics;
      if (result.hint.level === 1) {
        setFeedback(`Observe a peça que está em ${result.hint.square?.toUpperCase()}.`, "success", true);
      } else if (result.hint.level === 2) {
        setFeedback(`Considere estas casas: ${result.hint.squares.map((value) => value.toUpperCase()).join(", ")}.`, "success", true);
      } else if (result.hint.chess_move) {
        const move = result.hint.chess_move;
        setFeedback(`Movimento sugerido: ${move.from.toUpperCase()} → ${move.to.toUpperCase()}.`, "success", true);
      }
      renderMetrics(result.metrics);
    });
  }

  async function resetSession() {
    await withBusy(async () => {
      state.session = await api(`/api/v1/sessions/${sessionId}/reset`, { method: "POST" });
      state.selected = null;
      setFeedback("Seção reiniciada. Tente uma estratégia diferente.", "", true);
      render();
    });
  }

  async function withBusy(action) {
    if (state.busy) return;
    state.busy = true;
    render();
    try { await action(); } catch (error) { showError(error); }
    finally { state.busy = false; render(); }
  }

  async function api(url, options = {}) {
    const response = await fetch(url, {
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

  function launcherUrl() {
    const raw = String(window.CHESS_LAUNCHER_URL || "").trim();
    if (!raw) return "";
    try {
      const url = new URL(raw);
      if (url.protocol !== "http:" && url.protocol !== "https:") return "";
      return url.origin;
    } catch {
      return "";
    }
  }

  function parseSessionId(pathname) {
    const match = pathname.match(/^\/play\/([0-9a-f-]+)$/i);
    if (!match || !SESSION_ID_RE.test(match[1])) return null;
    return match[1];
  }

  function parseFen(fen) {
    const result = {};
    fen.split(" ")[0].split("/").forEach((row, index) => {
      let file = 0;
      for (const token of row) {
        if (/\d/.test(token)) file += Number(token);
        else { result[`${"abcdefgh"[file]}${8 - index}`] = token; file += 1; }
      }
    });
    return result;
  }

  function renderCaptured(initialFen, currentFen) {
    const initial = Object.values(parseFen(initialFen));
    const current = Object.values(parseFen(currentFen));
    const missing = [];
    for (const piece of initial) {
      const index = current.indexOf(piece);
      if (index >= 0) current.splice(index, 1); else if (piece.toLowerCase() !== "k") missing.push(pieces[piece]);
    }
    $("#captured").textContent = missing.length ? `Capturadas: ${missing.join(" ")}` : "Nenhuma peça capturada";
  }

  function renderMetrics(metrics) {
    $("#metric-moves").textContent = metrics.moves;
    $("#metric-correct").textContent = metrics.correct_moves;
    $("#metric-hints").textContent = metrics.hints_used;
  }

  function setFeedback(key, style = "", literal = false) {
    const feedback = $("#feedback");
    feedback.className = `feedback ${style}`;
    feedback.textContent = literal ? key : (messages[key] || key.replaceAll("_", " "));
  }

  function showError(error) {
    const toast = $("#toast");
    toast.textContent = error.message || "Não foi possível concluir a operação.";
    toast.hidden = false;
    setTimeout(() => { toast.hidden = true; }, 5000);
  }

  function coord(kind, text) {
    const value = document.createElement("span");
    value.className = `coord ${kind}`;
    value.textContent = text;
    return value;
  }

  function pieceName(piece) {
    return ({ p: "peão preto", n: "cavalo preto", b: "bispo preto", r: "torre preta", q: "dama preta", k: "rei preto", P: "peão branco", N: "cavalo branco", B: "bispo branco", R: "torre branca", Q: "dama branca", K: "rei branco" })[piece];
  }
})();
