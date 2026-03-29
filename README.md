# CellWar

A turn-based strategy game played on a grid. Players command units, build fortifications, and construct factories to overwhelm their opponents. The last player standing wins.

**Site:** cellwar.gg

---

## Table of Contents

- [Game Rules](#game-rules)
- [Architecture](#architecture)
- [Project Structure](#project-structure)
- [Building & Running](#building--running)

---

## Game Rules

### The Board

A rectangular N×M grid of cells. No obstacles or special terrain — pure strategy.

### Players

- Minimum 2 players; typically 2, with support for 3+
- Players take turns in a fixed order that never changes
- No alliances — every player is an enemy

### Rounds

A **round** is complete when every player has taken one turn. Within a turn, the active player moves all their units in any order, with one constraint: **combat units** (infantry + fortifications) must all act before **factories**. Every unit must act exactly once — passing is not allowed.

### Units

#### Infantry

- **Visibility radius:** 1 cell
- **Actions:**
  - **Move** `(dx, dy)` — move to an adjacent cell (up to 8 directions)
    - Empty cell → moves there
    - Enemy cell → attack (see combat rules)
    - Own unit → invalid
    - Out of bounds → invalid
  - **Stay** `(0, 0)` → transforms into a Fortification (counter = 1)

#### Fortification

- **Visibility radius:** 2 cells
- Has a standing counter: 1, 2, 3. At 3 it automatically becomes a Factory.
- **Actions:**
  - **Stay** `(0, 0)` → counter +1. At 3 → becomes a Factory
  - **Move** `(dx, dy)` → reverts to Infantry first, then Infantry rules apply
  - **Attack** `(dx, dy)` → same as move, but targets an enemy cell

#### Factory (Academy)

- **Visibility radius:** 1 cell
- Spawns from a Fortification after 3 consecutive standing turns
- **Actions:**
  - **Produce** `(dx, dy)` → creates a new Infantry on a free adjacent cell
    - The new Infantry has `acted = true` and cannot move on its spawn turn
  - `(0, 0)` → **invalid** (factory must produce)
  - If no free adjacent cells exist when the turn ends → Factory **dies**

### Action API

Every unit action is a vector `(dx, dy)` where `dx, dy ∈ {-1, 0, 1}`. The engine interprets the vector based on unit type and target cell state:

| Vector | Unit | Result |
|--------|------|--------|
| `(0,0)` | Infantry | Becomes Fortification (counter=1) |
| `(0,0)` | Fortification | Counter +1 (at 3 → Factory) |
| `(0,0)` | Factory | Invalid |
| `(dx,dy)` | Infantry + empty | Move |
| `(dx,dy)` | Infantry + enemy | Attack |
| `(dx,dy)` | Fortification | Becomes Infantry, then Infantry rules |
| `(dx,dy)` | Factory + empty | Produce Infantry |

### Combat Rules

**Key principle:** A Fortification that moves first becomes Infantry, then Infantry rules apply. Infantry never stays on its origin cell — it either moves to the target or is destroyed.

| Attacker | Target | Result |
|----------|--------|--------|
| Infantry | Infantry | Enemy destroyed, attacker takes the cell |
| Infantry | Fortification (1st hit) | Attacker destroyed, Fortification → Infantry |
| Infantry | Fortification (2nd hit, already Infantry) | Enemy destroyed, attacker takes the cell |
| Infantry | Factory | Factory destroyed, attacker takes the cell |
| Fortification | anything | Becomes Infantry first, then Infantry rules above |

### Fog of War

- Each player sees only cells within the visibility radius of their units
- Everything else is **fog** (unknown state)
- When a unit moves, its old area stays visible until the turn ends; its new area becomes visible immediately
- On the next turn, only the area around the unit's new position is visible

### Victory

- The last player with living units wins
- A draw is impossible

### Player Disconnection

- **Disconnect during own turn:** the game rolls back to the snapshot taken after the previous player's turn; all disconnected player's units are removed; the turn passes to the next player
- **Disconnect outside own turn:** units are removed immediately
- **Connection loss:** server waits for a timeout, then treats it as a disconnect
- If 1 player remains → that player wins
- If 2+ remain → game continues without the disconnected player

### Snapshots

The server stores a snapshot after each player's turn. A snapshot includes: all unit positions, fog-of-war state for each player, and fortification counters. Snapshots are used for rollback on disconnection.

---

## Architecture

```
[Browser Client]     [Python AI Client]
       |                     |
       └──────────┬──────────┘
                  ↓
               [Nginx]
                  ↓
           [Game Server]
           (Rust / Actix-web)
                  ↓
           [Game Engine]
           (Rust library)
```

### Components

#### 1. Game Engine (`/engine`)

A pure Rust library crate with zero network or I/O dependencies.

- **Input:** game state + action
- **Output:** new state + result

**Core types:**

```rust
GameConfig    // board dimensions, player count, starting positions
GameState     // complete game state (server-side only)
PlayerView    // fog-of-war state for a specific player
Action        // { unit_id, dx: i32, dy: i32 }
UnitType      // Infantry | Fortification { turns_standing } | Factory
Cell          // Empty | Unit(unit_id)
TileVisibility // Visible(Cell) | LastKnown(Cell) | Fog
Snapshot      // state snapshot for rollback
GameError     // InvalidAction | NotYourTurn | UnitAlreadyActed | ...
```

**Core functions:**

```rust
fn new_game(config: GameConfig) -> GameState
fn get_valid_actions(state: &GameState, player_id: u8, unit_id: u32) -> Vec<Action>
fn apply_action(state: &mut GameState, player_id: u8, unit_id: u32, action: Action) -> Result<(), GameError>
fn end_player_turn(state: &mut GameState, player_id: u8) -> Result<(), GameError>
fn get_visible_state(state: &GameState, player_id: u8) -> PlayerView
fn get_winner(state: &GameState) -> Option<u8>
fn remove_player(state: &mut GameState, player_id: u8)
fn rollback_to_snapshot(state: &mut GameState, snapshot_index: usize)
```

**Status:** ✅ Complete — 23 unit tests passing

#### 2. Game Server (`/server`)

Rust + Actix-web. Manages game sessions in memory (no persistence in v1). Handles multiple concurrent games and players.

**REST API:**

```
POST /game/create              — create a session, return game_id
POST /game/{id}/join           — join as a player, return player_token
GET  /game/{id}/state          — get PlayerView (fog-of-war applied)
POST /game/{id}/action         — submit a unit action
POST /game/{id}/end_turn       — end current player's turn
WS   /game/{id}/ws             — subscribe to real-time state updates
```

**Authentication:** simple token issued on `/join`, passed as `X-Player-Token` header.

**WebSocket messages (server → client):**

```json
{ "type": "StateUpdate", "view": { ... } }
{ "type": "YourTurn" }
{ "type": "GameOver", "winner": 1 }
{ "type": "PlayerDisconnected", "player_id": 2 }
{ "type": "Rollback" }
```

**Disconnection handling:** on WebSocket close, a 30-second timer starts. If the player doesn't reconnect, `remove_player` + `rollback_to_snapshot` are called automatically.

**Status:** 🔧 Stub (routes return 501)

#### 3. Nginx (`/nginx`)

- Proxies API requests to the Game Server
- Serves the browser client's static files
- Handles WebSocket upgrade headers

**Status:** 📋 Planned

#### 4. Browser Client (`/client`)

Vanilla JS single-page application. No frameworks.

**Screens:**
- **Lobby** — create a game (set board size), or join by game ID
- **Game** — interactive grid, unit status panel, end-turn button

**Unit icons (inline SVG):**
- Infantry → ⚔ sword
- Fortification → 🛡 shield with standing counter (1 or 2)
- Factory → ⚙ gear

Unit color = player color (player 1 = blue, player 2 = red, player 3 = green, …).

**Interaction model:**
- Left-click a unit to select it; Tab / Shift+Tab to cycle through available units
- Right-click a neighboring cell to set the move vector (first click = preview, second click = confirm)
- Right-click the unit's own cell → `(0,0)` stay vector
- Arrow keys to navigate the vector; Enter to confirm

**Move preview:** shown immediately after the vector is set, before confirmation. Shows skulls 💀 on units that will be destroyed, and type changes for fortifications hit for the first time.

**Status:** 📋 Planned

#### 5. AI Client (`/ai_client`)

Python script. Connects to the server as a regular player using the same REST/WebSocket API.

Two variants planned:
- **Random bot** — picks a random valid action for each unit
- **RL bot** — trained model using `gymnasium`

**Status:** 📋 Planned

#### 6. Console CLI (`/cli`)

A standalone Rust binary for local play without a server. Connects directly to the engine library.

**Features:**
- Configure map size and number of players at startup
- Each player can be Human or a Python bot subprocess
- ASCII board rendering with ANSI colors
- Human input: select unit by coordinates, choose `(dx,dy)` from listed valid moves
- Python bot protocol: JSON over stdin/stdout

**Bot protocol:**

Server → bot (stdin):
```json
{ "type": "state", "view": { "my_units": [ { "id": 1, "valid_actions": [...] } ] } }
```

Bot → server (stdout):
```json
{ "type": "action", "unit_id": 1, "dx": 0, "dy": 1 }
{ "type": "end_turn" }
```

**Status:** ✅ Complete

#### 7. Replay System (`/engine/src/replay.rs`)

Match replays saved as `.cwreplay` files (MessagePack binary format).

**Replay file structure:**
```
version, match_id, started_at, ended_at, winner
players: [ { player_id, name, is_ai } ]
config: GameConfig
events: [ { timestamp, round, player_id, event_type } ]
  event_type: Action { unit_id, dx, dy } | EndTurn | PlayerDisconnected | Rollback
```

**Playback** is handled entirely on the client side — the server only serves the file. The map is shown fully revealed (no fog of war) during replay.

Two playback modes:
- **Step-by-step** — ← → buttons, highlights the last-moved unit, jump to any moment
- **Real-time** — plays back with original timestamps, Pause/Resume button, speed slider (0.5×–4×), progress bar with seek

**Status:** 📋 Planned

---

## Project Structure

```
/
├── Cargo.toml              ← workspace (engine + server + cli)
├── engine/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs          ← all game logic and types
├── server/
│   ├── Cargo.toml
│   ├── Dockerfile
│   └── src/
│       └── main.rs
├── cli/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs
├── client/
│   └── index.html          ← single-page browser client
├── ai_client/
│   └── random_bot.py
├── nginx/
│   ├── nginx.conf
│   └── Dockerfile
├── tests/
│   └── integration_test.py
└── docker-compose.yml
```

---

## Building & Running

**Requirements:** Rust 1.75+, Cargo

```bash
# Build everything
cargo build

# Run all engine tests
cargo test -p cellwar-engine

# Play locally (CLI)
cargo run -p cellwar-cli
```

**CLI session example:**
```
=== CellWar CLI ===
Map size (NxM): 10x10
Number of players: 2
Player 1: [H]uman / [P]ython bot? H
Player 2: [H]uman / [P]ython bot? P
  Bot command: python3 ai_client/random_bot.py

=== Round 1 | Player 1's turn ===
   0  1  2  3  4  5
0  .  .  .  .  .  .
1  .  1I .  .  .  .
...
Select unit (x,y) or 'q' to quit: 1,1
Unit: Infantry at (1,1)
Valid moves: (0,0) (1,0) (0,1) (1,1) (-1,0) ...
Enter move (dx,dy): 1,0
```
