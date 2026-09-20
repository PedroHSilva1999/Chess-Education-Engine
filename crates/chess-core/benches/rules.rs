use chess_core::{ChessMove, ChessRulesEngine, Position, ShakmatyRules};
use criterion::{Criterion, criterion_group, criterion_main};

fn rules_benchmarks(criterion: &mut Criterion) {
    let rules = ShakmatyRules;
    let position = Position::initial();

    criterion.bench_function("fen parsing", |bencher| {
        bencher.iter(|| Position::from_fen(position.fen.clone()).unwrap())
    });
    criterion.bench_function("initial legal move generation", |bencher| {
        bencher.iter(|| rules.legal_moves(&position).unwrap())
    });
    criterion.bench_function("move validation and application", |bencher| {
        let chess_move = ChessMove::new("e2", "e4", None).unwrap();
        bencher.iter(|| rules.apply_move(&position, &chess_move).unwrap())
    });
}

criterion_group!(benches, rules_benchmarks);
criterion_main!(benches);
