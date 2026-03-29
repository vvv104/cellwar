# CellWar — Задачи для разработки (Claude Code)

Задачи выполняются строго по порядку. Каждая следующая задача зависит от предыдущей.
Перед началом каждой задачи прочитай CLAUDE.md — там полные правила игры и архитектура.

---

## ЗАДАЧА 1: Структура проекта

Создай базовую структуру Rust workspace:

```
/
├── Cargo.toml          ← workspace
├── engine/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── server/
    ├── Cargo.toml
    └── src/
        └── main.rs
```

`Cargo.toml` (workspace):
```toml
[workspace]
members = ["engine", "server"]
resolver = "2"
```

`engine/Cargo.toml`:
```toml
[package]
name = "cellwar-engine"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

`server/Cargo.toml`:
```toml
[package]
name = "cellwar-server"
version = "0.1.0"
edition = "2021"

[dependencies]
cellwar-engine = { path = "../engine" }
actix-web = "4"
actix-ws = "0.3"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }
dashmap = "5"
```

Убедись что `cargo build` проходит без ошибок.

---

## ЗАДАЧА 2: Типы данных движка

В `engine/src/lib.rs` определи все основные типы. Никакой логики пока — только типы и их реализация Display/Debug/Clone/Serialize/Deserialize.

```rust
// Координата на поле
pub struct Position { pub x: u32, pub y: u32 }

// Тип юнита
pub enum UnitType {
    Infantry,
    Fortification { turns_standing: u8 }, // 1, 2, 3 (на 3 → Factory)
    Factory,
}

// Юнит
pub struct Unit {
    pub id: u32,
    pub owner: u8,        // player_id
    pub unit_type: UnitType,
    pub position: Position,
    pub acted: bool,      // сходил ли в этом ходу
}

// Действие юнита — всегда вектор
// (0,0) = стоять, (dx,dy) = двигаться/атаковать/производить
// Движок интерпретирует исходя из типа юнита и целевой клетки
pub struct Action {
    pub unit_id: u32,
    pub dx: i32,  // -1, 0, 1
    pub dy: i32,  // -1, 0, 1
}

// Конфигурация игры
pub struct GameConfig {
    pub width: u32,
    pub height: u32,
    pub player_count: u8,
}

// Клетка поля
pub enum Cell {
    Empty,
    Unit(u32), // unit_id
}

// Полное состояние игры (только для сервера)
pub struct GameState {
    pub config: GameConfig,
    pub cells: Vec<Vec<Cell>>,  // [y][x]
    pub units: HashMap<u32, Unit>,
    pub next_unit_id: u32,
    pub players: Vec<u8>,              // активные игроки в порядке хода
    pub current_player_index: usize,
    pub round: u32,
    pub snapshots: Vec<Snapshot>,      // снапшоты после каждого хода игрока
}

// Снапшот для rollback
pub struct Snapshot {
    pub player_id: u8,
    pub round: u32,
    pub cells: Vec<Vec<Cell>>,
    pub units: HashMap<u32, Unit>,
}

// Видимое состояние для игрока (с туманом войны)
pub enum TileVisibility {
    Visible(Cell),    // видно прямо сейчас
    LastKnown(Cell),  // последнее известное состояние
    Fog,              // неизвестно
}

pub struct PlayerView {
    pub player_id: u8,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Vec<TileVisibility>>,
    pub my_units: Vec<Unit>,
    pub current_player: u8,
    pub round: u32,
    pub winner: Option<u8>,
}

// Ошибки
pub enum GameError {
    InvalidAction(String),
    NotYourTurn,
    UnitAlreadyActed,
    UnitNotFound,
    GameOver,
}
```

Убедись что всё компилируется.

---

## ЗАДАЧА 3: Логика движка — базовые функции

В `engine/src/lib.rs` реализуй следующие функции:

### `new_game(config: GameConfig) -> GameState`
- Создаёт пустое поле
- Расставляет начальные юниты (по одному пехотинцу на игрока)
- Позиции распределяются равномерно по полю (например для 2 игроков — левый верхний и правый нижний углы с отступом 1)
- Возвращает начальное состояние

### `get_valid_actions(state: &GameState, player_id: u8, unit_id: u32) -> Vec<Action>`
- Возвращает все допустимые действия для юнита в виде векторов `(dx, dy)`
- Пехотинец: `(0,0)` + все `(dx,dy)` ведущие на пустые/вражеские соседние клетки
- Укрепление: те же что у пехотинца (движок сам применит правила)
- Фабрика: все `(dx,dy)` ведущие на свободные соседние клетки (если нет — пустой список)
- Учитывает границы поля, клетки своих юнитов исключаются

### `get_visible_state(state: &GameState, player_id: u8) -> PlayerView`
- Вычисляет какие клетки видит игрок
- Пехотинец: радиус 1, Укрепление: радиус 2, Фабрика: радиус 1
- Возвращает PlayerView с туманом войны

Убедись что все функции компилируются и покрыты unit-тестами:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_new_game_places_units() { ... }
    
    #[test]
    fn test_valid_actions_infantry() { ... }
    
    #[test]
    fn test_fog_of_war() { ... }
}
```

---

## ЗАДАЧА 4: Логика движка — применение действий

Реализуй функцию:

### `apply_action(state: &mut GameState, player_id: u8, unit_id: u32, action: Action) -> Result<(), GameError>`

Обрабатывай все случаи согласно правилам в CLAUDE.md. Вектор `(dx, dy)` интерпретируется так:

**`(0, 0)` для Пехотинца:**
- Превращается в Укрепление с turns_standing = 1

**`(dx, dy)` для Пехотинца:**
- Целевая клетка пуста → перемещение
- Целевая клетка содержит врага-пехотинца → враг уничтожен, пехотинец занимает клетку
- Целевая клетка содержит врага-укрепление → атакующий уничтожен, укрепление → пехотинец
- Целевая клетка содержит врага-фабрику → фабрика уничтожена, пехотинец занимает клетку
- Целевая клетка содержит своего → GameError::InvalidAction

**`(0, 0)` для Укрепления:**
- turns_standing < 3 → turns_standing += 1
- turns_standing == 3 → превращается в Фабрику

**`(dx, dy)` для Укрепления:**
- Укрепление сначала становится Пехотинцем, затем применяются правила пехотинца выше

**`(dx, dy)` для Фабрики:**
- Целевая клетка свободна → создаёт нового Пехотинца с acted = true
- `(0, 0)` → GameError::InvalidAction (фабрика не может стоять)

После каждого действия: unit.acted = true

### `end_player_turn(state: &mut GameState, player_id: u8) -> Result<(), GameError>`
- Проверяет что все юниты игрока сходили (acted == true)
- Убивает фабрики у которых нет свободных соседних клеток
- Делает снапшот текущего состояния
- Переходит к следующему игроку
- Сбрасывает acted = false для юнитов следующего игрока
- Проверяет победителя

### `get_winner(state: &GameState) -> Option<u8>`
- Возвращает player_id если у него остались юниты а у всех остальных нет

Покрой тестами все боевые случаи:
```rust
#[test]
fn test_infantry_attacks_infantry() { ... }

#[test]
fn test_infantry_attacks_fortification_first_hit() { ... }

#[test]
fn test_two_infantry_destroy_fortification() { ... }

#[test]
fn test_fortification_becomes_factory() { ... }

#[test]
fn test_factory_produces_unit() { ... }

#[test]
fn test_factory_dies_no_space() { ... }
```

---

## ЗАДАЧА 5: Логика движка — отключение игрока

Реализуй:

### `remove_player(state: &mut GameState, player_id: u8)`
- Удаляет все юниты игрока с поля
- Удаляет игрока из списка активных игроков
- Если текущий ход был этого игрока — переходит к следующему

### `rollback_to_snapshot(state: &mut GameState, snapshot_index: usize)`
- Восстанавливает состояние из снапшота
- Сохраняет список активных игроков (не восстанавливает выбывших)

Тесты:
```rust
#[test]
fn test_remove_player_during_turn() { ... }

#[test]
fn test_rollback_restores_state() { ... }

#[test]
fn test_winner_after_disconnect() { ... }
```

---

## ЗАДАЧА 6: Game Server — базовая структура

В `server/src/main.rs` создай Actix-web сервер:

**Структуры данных сервера:**
```rust
// Активная игровая сессия
struct GameSession {
    state: Arc<Mutex<GameState>>,
    players: HashMap<u8, PlayerConnection>,
}

struct PlayerConnection {
    token: String,
    tx: Option<mpsc::Sender<ServerMessage>>, // WebSocket канал
    connected: bool,
    disconnect_timer: Option<JoinHandle<()>>,
}

// Сообщения сервера → клиенту
enum ServerMessage {
    StateUpdate(PlayerView),
    YourTurn,
    GameOver { winner: u8 },
    PlayerDisconnected { player_id: u8 },
    Rollback,
}
```

**Маршруты:**
```
POST /game/create
POST /game/{id}/join
GET  /game/{id}/state
POST /game/{id}/action
POST /game/{id}/end_turn
WS   /game/{id}/ws
```

Реализуй `POST /game/create` и `POST /game/{id}/join` полностью.
Остальные маршруты — заглушки возвращающие 501 Not Implemented.

---

## ЗАДАЧА 7: Game Server — игровые маршруты

Реализуй оставшиеся маршруты:

### `GET /game/{id}/state`
- Требует заголовок `X-Player-Token`
- Возвращает `PlayerView` для этого игрока (с туманом войны)

### `POST /game/{id}/action`
```json
{
  "unit_id": 1,
  "action": { "type": "Move", "direction": "N" }
}
```
- Проверяет токен
- Проверяет что сейчас ход этого игрока
- Применяет действие через движок
- Рассылает обновление всем игрокам через WebSocket

### `POST /game/{id}/end_turn`
- Завершает ход игрока
- Уведомляет следующего игрока через WebSocket

### `WS /game/{id}/ws`
- Принимает WebSocket соединение
- При подключении: шлёт текущее состояние
- При отключении: запускает таймер (30 секунд), потом вызывает remove_player + rollback

---

## ЗАДАЧА 8: Nginx конфигурация

Создай файл `nginx/nginx.conf`:

```nginx
events {}

http {
    upstream game_server {
        server localhost:8080;
    }

    server {
        listen 80;

        # Статика клиента
        location / {
            root /var/www/client;
            index index.html;
            try_files $uri $uri/ /index.html;
        }

        # API сервера
        location /api/ {
            proxy_pass http://game_server/;
            proxy_http_version 1.1;
            proxy_set_header Host $host;
        }

        # WebSocket
        location /ws/ {
            proxy_pass http://game_server/;
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
            proxy_set_header Host $host;
            proxy_read_timeout 3600s;
        }
    }
}
```

Создай `nginx/Dockerfile`:
```dockerfile
FROM nginx:alpine
COPY nginx.conf /etc/nginx/nginx.conf
```

---

## ЗАДАЧА 9: Браузерный клиент

Создай `client/index.html` — одностраничное приложение на Vanilla JS.

**Экраны:**
1. **Лобби**: поле ввода размера карты, кнопка "Создать игру", поле ввода game_id + кнопка "Подключиться"
2. **Игра**: сетка поля, панель состояния, кнопка "Завершить ход"

---

**Иконки юнитов (SVG встроенные):**
- Пехотинец → ⚔️ меч
- Укрепление → 🛡️ щит с цифрой счётчика (1, 2) внутри
- Фабрика → ⚙️ шестерёнка

Цвет иконки = цвет игрока (игрок 1 = синий, игрок 2 = красный, игрок 3 = зелёный и т.д.)

**Состояния юнитов:**
- Обычный → полная яркость, кликабелен
- Уже сходил (acted = true) → иконка тусклая (opacity: 0.35), некликабелен
- Фабрика пока недоступна (боевые ещё не все сходили) → тусклая, некликабелна
- Выбран → подсветка клетки (яркая рамка)

**Цвета клеток:**
- Туман войны → тёмно-серый (#2a2a2a)
- Пустая видимая → светло-серый (#e0e0e0)
- Последнее известное состояние → серый (#999), слегка затемнён

---

**Выбор юнита (мышь и клавиатура единая модель):**
- Левый клик на своего юнита (не acted, доступен) → выбрать
- Tab → переключиться на следующий доступный юнит (пропускать acted и недоступные фабрики)
- Shift+Tab → переключиться на предыдущий
- Клик на пустое место → снять выбор

**Установка вектора хода (после выбора юнита):**

Мышь:
1. Первый правый клик на соседнюю клетку → установить вектор, показать превью
2. Правый клик на другую клетку → сменить вектор, превью обновляется
3. Правый клик на ту же клетку (вектор уже установлен) → подтвердить ход (как Enter)
4. Правый клик на клетку самого юнита → вектор (0,0) "стоять" + превью. Повторный правый клик → подтвердить. Только для боевых юнитов.

Клавиатура:
1. Первое нажатие стрелки → вектор появляется в том направлении. Недопустимые направления (край карты, свой юнит) игнорируются.
2. Последующие стрелки → перемещают вектор по сетке 3x3. Недопустимые клетки пропускаются.
3. Для боевого юнита: стрелки могут вернуть вектор на (0,0) → "стоять".
4. Для фабрики: вернуть вектор на (0,0) после первого нажатия нельзя.
5. Enter → подтвердить ход по текущему вектору.

**Превью хода (показывается сразу при установке вектора, до подтверждения):**

Атака пехотинцем или укреплением:
- Цель — пехотинец врага → череп 💀 на клетке цели
- Цель — укрепление врага, первый удар → иконка цели меняется на меч (станет пехотинцем) + череп 💀 на атакующем
- Цель — укрепление врага, второй удар (уже пехотинец после первого удара) → череп 💀 на цели
- Цель — фабрика врага → череп 💀 на цели

Фабрика производит:
- Если производство заблокирует последнюю свободную клетку своей другой фабрики → череп 💀 на той фабрике
- Только для своих фабрик (чужие не показываем)

Умирающая фабрика (уже сейчас нет свободных клеток):
- Иконка заменяется на череп 💀 прямо на поле
- Недоступна для выбора

**Визуализация вектора:**
- Стрелка от центра клетки юнита до целевой клетки
- Целевая клетка подсвечивается рамкой
- При (0,0) → точка на юните, рамка вокруг его клетки

---

**WebSocket:**
- При получении StateUpdate → перерисовать поле полностью
- При получении YourTurn → показать уведомление "Ваш ход"
- При получении GameOver → показать баннер с победителем
- При получении Rollback → показать уведомление "Игрок вышел, ход пересчитан"
- При получении PlayerDisconnected → показать уведомление

---

## ЗАДАЧА 10: AI клиент (случайный бот)

Создай `ai_client/random_bot.py` — простейший AI который делает случайные допустимые ходы.

```python
import requests
import random
import time

SERVER = "http://localhost:8080"

def join_game(game_id):
    r = requests.post(f"{SERVER}/game/{game_id}/join")
    return r.json()["player_token"], r.json()["player_id"]

def get_state(game_id, token):
    r = requests.get(f"{SERVER}/game/{game_id}/state",
                     headers={"X-Player-Token": token})
    return r.json()

def send_action(game_id, token, unit_id, dx, dy):
    r = requests.post(f"{SERVER}/game/{game_id}/action",
                      headers={"X-Player-Token": token},
                      json={"unit_id": unit_id, "dx": dx, "dy": dy})
    return r.json()

def end_turn(game_id, token):
    requests.post(f"{SERVER}/game/{game_id}/end_turn",
                  headers={"X-Player-Token": token})

def play(game_id):
    token, player_id = join_game(game_id)
    print(f"Joined as player {player_id}")

    while True:
        state = get_state(game_id, token)

        if state.get("winner"):
            print(f"Game over. Winner: {state['winner']}")
            break

        if state["current_player"] != player_id:
            time.sleep(0.5)
            continue

        # Ходим каждым своим юнитом
        for unit in state["my_units"]:
            if not unit["acted"]:
                actions = unit["valid_actions"]  # список {"dx": .., "dy": ..}
                if actions:
                    action = random.choice(actions)
                    send_action(game_id, token, unit["id"], action["dx"], action["dy"])

        end_turn(game_id, token)

if __name__ == "__main__":
    import sys
    play(sys.argv[1])
```

---

## ЗАДАЧА 11: Docker Compose

Создай `docker-compose.yml` для запуска всего стека:

```yaml
version: '3.8'
services:
  server:
    build:
      context: .
      dockerfile: server/Dockerfile
    ports:
      - "8080:8080"

  nginx:
    build: ./nginx
    ports:
      - "80:80"
    volumes:
      - ./client:/var/www/client
    depends_on:
      - server
```

Создай `server/Dockerfile`:
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release -p cellwar-server

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/cellwar-server /usr/local/bin/
CMD ["cellwar-server"]
```

---

## ЗАДАЧА 12: Интеграционные тесты

Создай `tests/integration_test.py` на Python:

- Тест: создать игру, два бота играют до победы
- Тест: один бот отключается, второй побеждает
- Тест: rollback при отключении во время хода

```python
def test_full_game():
    # Создать игру
    # Подключить двух random_bot
    # Убедиться что игра завершается победителем
    pass

def test_disconnect_wins():
    # Подключить двух игроков
    # Один отключается
    # Убедиться что второй получает победу
    pass
```

---

## ЗАДАЧА 13: Система реплеев

### Формат файла (.cwreplay)

Бинарный формат MessagePack. Расширение `.cwreplay`.

Структура:

```rust
struct Replay {
    // Метаданные
    version: u8,                    // версия формата
    match_id: String,               // уникальный id матча
    started_at: u64,                // unix timestamp начала
    ended_at: u64,                  // unix timestamp конца
    winner: u8,                     // player_id победителя
    players: Vec<PlayerMeta>,       // имена и id игроков

    // Конфигурация
    config: GameConfig,             // размер поля, кол-во игроков

    // Все ходы матча
    events: Vec<ReplayEvent>,
}

struct PlayerMeta {
    player_id: u8,
    name: String,                   // имя или "Bot" или "Anonymous"
    is_ai: bool,
}

struct ReplayEvent {
    timestamp: u64,                 // unix timestamp события
    round: u32,
    player_id: u8,
    event_type: ReplayEventType,
}

enum ReplayEventType {
    Action { unit_id: u32, dx: i32, dy: i32 },
    EndTurn,
    PlayerDisconnected,
    Rollback,
}
```

### Сохранение на сервере

- Сервер пишет события в `ReplayRecorder` на протяжении всего матча
- По окончании матча (победа или все отключились) — сериализует в MessagePack и сохраняет файл
- Файл именуется: `{match_id}.cwreplay`
- Добавить endpoint: `GET /game/{id}/replay` — скачать файл реплея после завершения матча

### Воспроизведение реплея

Реплей воспроизводится на клиенте — сервер только отдаёт файл. Клиент сам прогоняет события через движок и рендерит результат.

**Важно:** при воспроизведении реплея карта полностью открыта — туман войны отсутствует, видны все юниты всех игроков одновременно.

**Два режима воспроизведения** доступны одновременно (переключатель в UI):

*Режим 1 — Шаг за шагом:*
- Кнопки ← → для перехода между ходами
- Показывается какой игрок ходит, номер раунда
- Подсвечивается юнит который только что сходил
- Можно перейти к любому моменту матча

*Режим 2 — Реальное время:*
- Воспроизводится с реальными задержками между ходами (из timestamp)
- Кнопка Pause/Resume
- Слайдер скорости: 0.5x, 1x, 2x, 4x
- Прогресс-бар матча, можно кликнуть для перемотки

### Где смотреть реплей

**Вариант А — В игровом клиенте:**
- Кнопка "Watch Replay" после окончания матча
- Кнопка "Load Replay" в лобби — загрузить `.cwreplay` файл с диска

**Вариант Б — Отдельная страница в браузере:**
- Маршрут `/replay/{match_id}` — загружает реплей с сервера и воспроизводит
- Можно поделиться ссылкой

### Задача для Claude Code

1. В `engine/` добавить `replay.rs`:
   - Типы `Replay`, `ReplayEvent`, `ReplayEventType`, `PlayerMeta`
   - `ReplayRecorder` — накапливает события во время матча
   - `fn record_action(recorder, round, player_id, unit_id, dx, dy)`
   - `fn record_end_turn(recorder, round, player_id)`
   - `fn finalize(recorder, winner, ended_at) -> Replay`
   - `fn save_replay(replay: &Replay, path: &str) -> Result<(), Error>` — сериализация в MessagePack
   - `fn load_replay(path: &str) -> Result<Replay, Error>` — десериализация

2. В `server/` подключить `ReplayRecorder` к каждой сессии, вызывать `finalize` и сохранять файл по окончании матча. Добавить endpoint `GET /game/{id}/replay`.

3. В `client/` добавить:
   - Страницу `/replay/{match_id}` 
   - Кнопку "Watch Replay" после конца матча
   - Кнопку "Load Replay" в лобби
   - Плеер с двумя режимами (шаг за шагом + реальное время)
   - Открытая карта без тумана войны
   - Подсветка последнего сходившего юнита

Зависимость для MessagePack в `engine/Cargo.toml`:
```toml
rmp-serde = "1"
```

---

## ЗАДАЧА 14: Консольный тестовый клиент

Отдельный бинарник `cellwar-cli` в `/cli`. Подключается напрямую к движку без сервера и сети.

### Настройка игры при запуске

```
=== CellWar CLI ===
Map size (NxM): 10x10
Number of players: 2

Player 1: [H]uman / [P]ython bot? H
Player 2: [H]uman / [P]ython bot? P
  Bot command: python3 bots/random_bot.py
```

### Отображение поля в консоли

Поле рисуется ASCII-символами. Каждая клетка — 2 символа (символ + цифра или пробел):

```
   0  1  2  3  4  5  6  7  8  9
0  .  .  .  .  .  .  .  .  .  .
1  .  .  .  .  .  .  .  .  .  .
2  .  1I .  .  .  .  .  .  .  .
3  .  .  .  .  .  .  .  .  .  .
4  .  .  .  .  .  .  .  .  .  .
5  .  .  .  .  .  .  .  .  .  .
6  .  .  .  .  .  .  .  .  2F .
7  .  .  .  .  .  .  .  .  .  .
```

Обозначения:
- `.` — пустая клетка
- `?` — туман войны
- `1I` — игрок 1, пехотинец (Infantry)
- `1F1` — игрок 1, укрепление (Fortification), счётчик 1
- `1A` — игрок 1, фабрика (Academy)
- `1I*` — юнит уже сходил в этом ходу (звёздочка)

Цвета через ANSI escape codes если терминал поддерживает:
- Игрок 1 — синий
- Игрок 2 — красный
- Игрок 3 — зелёный
- Туман — тёмно-серый

### Ход человека

```
=== Round 3 | Player 1's turn ===
Units: 1I@(2,2) 1F2@(3,4) 1A@(5,5)*

Select unit (x,y) or 'q' to quit: 2,2
Unit: Infantry at (2,2)
Valid moves: (1,1) (2,1) (3,1) (1,2) (3,2) (1,3) (2,3) (3,3) [0,0=stay]
Enter move (dx,dy): 0,1

Unit: Fortification[2] at (3,4)
Valid moves: ...
Enter move (dx,dy): 0,0

All combat units done. Factories:
Unit: Factory at (5,5) — already acted this turn.

Turn ended.
```

### Ход Python бота

Python бот запускается как subprocess. Общение через stdin/stdout в формате JSON:

Сервер → бот (stdin):
```json
{"type": "state", "view": {...}}
```

Бот → сервер (stdout):
```json
{"type": "action", "unit_id": 1, "dx": 0, "dy": 1}
```

После всех юнитов:
```json
{"type": "end_turn"}
```

Бот получает `PlayerView` (с туманом войны) — честная игра.

### Файл `/cli/Cargo.toml`

```toml
[package]
name = "cellwar-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
cellwar-engine = { path = "../engine" }
serde_json = "1"
```

### Добавить в workspace `/Cargo.toml`

```toml
[workspace]
members = ["engine", "server", "cli"]
```

### Запуск

```bash
cargo run -p cellwar-cli
```

---

## Порядок выполнения

1. → 2. → 3. → 4. → 5. (движок полностью)
**14. (консольный клиент — рекомендуется сразу после задачи 5, удобно для отладки движка)**
6. → 7. (сервер)
8. (nginx)
9. (клиент)
10. (AI клиент)
11. (docker)
12. (тесты)
13. (реплеи)

После каждой задачи: `cargo test` должен проходить без ошибок.
