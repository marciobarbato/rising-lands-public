# Better unit paths ("Caminhos melhores")

> **Resumo em português.** Regra opcional do motor, desligada por padrão: bit 0 de `regras`
> (`RL_REGRA_CAMINHOS_MELHORES`). Ligada, as unidades planejam o caminho inteiro antes de andar (A* com inteiros, sem
> sorteio), um grupo recebe uma célula de destino por unidade, e uma unidade bloqueada espera e recalcula em vez de
> desistir. Combate, IA, velocidades e o teste de passagem não mudam: só as decisões de para onde ir, quando esperar e
> quando parar. Desligada, o jogo é o original byte a byte. A regra vale a partir do início de uma fase; o save guarda as
> regras da partida. No macOS: menu **Jogo → Caminhos melhores** ou o item CAMINHOS MELHORES da tela de Opções. No
> Windows: o item CAMINHOS MELHORES da tela de Opções.

## What it is

An optional rule set for unit movement, **off by default**. It is bit 0 of `RlMissaoInicio.regras`
(`RL_REGRA_CAMINHOS_MELHORES` in `engine/include/rising_engine.h`); bit 1 is the Shift+click order queue. The code is
`engine/crates/rl-core/src/modern_path.rs`.

With the rule off, every hook takes the original branch, so the game is the original byte for byte: same code path,
same memory, same random draws, same saves.

## What changes with the rule on

Units still advance one cell at a time through the original step code and occupancy grids, so they never overlap.
Combat, AI strategy, speeds and what a unit may stand on are unchanged. Only these decisions change:

- **Plan first.** An order computes a full path before the first step: an integer A* on the unit's footprint (1x1, or
  2x2 for war machines), 10 per step, +1 per heading change, +40 through a stationary friend; moving units are ignored
  at plan time; stationary enemies block, except what a crusher runs over. At most 16,384 expansions per search. A
  frame spends at most 24,576 planner expansions (a search counts at least 3,072, so cheap searches stay at 8 per
  frame); a plan that does not fit waits for the next frame. This budget restarts every frame and is never saved.
- **Unreachable click.** The unit goes to the reachable cell closest to the target instead of stopping.
- **Blocked on the way.** A unit in front that is moving, or still has an order of its own, is traffic: wait 2 frames
  and try again. One with no order, head-on, or in a cycle of units waiting for each other is a hard block: shove it if
  it is an idle friend, wait 4, 8, 16 frames and replan. The fourth hard block in a row, or one next to the goal, ends
  the order where the unit is.
- **Group orders.** Each unit gets its own goal cell, taken breadth-first around the clicked cell; 2x2 machines are
  placed first, on cells where their whole footprint is free.
- **Gates.** An open gate is crossed and a closed one blocks (the original planner never routes through a gate).

It applies to every unit moved by the original DeplaceEngin routine, the player's, the AI's and monsters'. The AI's own
path queries keep the original planner.

## Determinism and saves

- The planner draws **no** random value (no game `Random`, no CRT `rand`, no permutation cursor). A unit test checks the
  module names no random source, and another checks the game seed and permutation cursor are unchanged after planning.
- No floating point and no hash maps: fixed arrays and a heap ordered by `(f, h, insertion counter)`. The same orders
  give the same bytes.
- Per-unit state lives in the unit's original fields and route buffer, so a save carries the paths with no new data.
- A world with rules writes **save schema 3** (the rules in an extra section); a world without rules still writes
  schema 2, byte-identical to before. An engine without this feature refuses schema 3, so a modern save never loads as
  original, and an original save never turns modern.

## When the switch takes effect

Only when a stage starts: a new stage, the next stage of a campaign, or a restart. Never in the middle of a match, and
never on load: a loaded save keeps the rules it was saved with. `rl_regras_definir` works only before the stage's first
frame and returns `RL_INVALID_ARGUMENT` afterwards; `rl_regras` reads the running world's rules. Later stages of a
campaign inherit them.

## Hosts

- **macOS:** menu **Jogo → Caminhos melhores**, or the CAMINHOS MELHORES item of the Opções screen (both drive the same
  setting). It applies from the next stage start.
- **Windows:** the CAMINHOS MELHORES item of the Opções screen (kept in `opcoes.ini`, off by default as on macOS). It
  applies to worlds created afterwards; `windows/parity.sh` checks the rule in the created world.

## How it was checked

In the private research repository, before this sync:

- Engine unit tests on a synthetic 16x16 world (no game data): shortest path around obstacles, unreachable goal, open
  and closed gates, 2x2 footprints, reproducibility, no random draws; save and ABI tests for schema 3 and the rule lock.
  They are in this repository and run with `cargo test`.
- With the rule off, measurement sweeps over a stage-start state (6,401 single-unit trips, 366 group orders, 24 maze
  crossings) matched the unmodified engine field by field, and the pinned bot census stayed identical.
- With the rule on, on the same state: no abandoned orders and no units that never stop (the original had 20 abandons
  and 4 endless orders in 366 group orders), group settle time p90 1.34x the ideal (original 1.57x), worst 1.97x
  (original 8.93x). Bridges, gates and 2x2 machines were measured on three more stages.

The measurement tool and its traces are not published: they run on states of the original stages, which come from the
CD.

Frame time (logic only, no renderer), MONDE17 (256x256, the largest map), 39 land units, rule on, 2026-11-13:

| Scenario | Mean | p99 | Worst frame |
|---|---:|---:|---:|
| Long orders across open ground | 0.64 ms | 0.99 ms | 4.6 ms |
| A new order every 200 frames | 0.71 ms | 3.4 ms | 6.3 ms |
| The stage start (units penned in, goals unreachable) | 0.52 ms | 2.1 ms | 3.7 ms |

No frame took more than 8.3 ms (the budget at 4x speed). The first measurement found bursts of 36–44 ms right after each
group order; the planner fix (shared buffers, the per-frame expansion budget, an early stop for attack approaches)
removed them without changing the routes in the measured scenarios. With the rule off the original code runs: about
0.6 ms mean, under 3 ms worst.
