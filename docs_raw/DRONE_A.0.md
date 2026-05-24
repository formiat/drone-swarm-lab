# Drone/Fleet/Swarm Systems: макро-план и симуляция без лаборатории

Дата фиксации: 2026-05-15

## Контекст

Цель: подойти к теме дронов, флотов и роев не как к маленькой учебной песочнице, а как к серьезному крупному инженерному проекту.

Ограничение: нет мастерской, лаборатории, физических дронов и полигона.

Вывод: это не мешает заниматься темой серьезно, если объектом разработки сделать не "игрушечный симулятор полета", а:

> Mission-level digital twin для автономных drone/fleet/swarm-систем.

То есть платформу моделирования миссий, флота, связи, среды, отказов, распределения задач, ограничений и метрик.

## Главный разворот

Предыдущий подход "начать снизу с boids и маленькой песочницы" полезен для обучения, но может ощущаться несерьезным.

Более взрослый подход:

1. Начать с макро-карты применений.
2. Разложить возможные архитектуры.
3. Выделить слои системы.
4. Выбрать несколько reference missions.
5. Спроектировать модели мира, флота, связи, миссий и безопасности.
6. Строить симуляционную платформу, которая сравнивает архитектуры и стратегии.

Цель проекта:

> Distributed Autonomous Mission System for Drone Fleets and Swarms.

По-русски:

> Система моделирования и управления распределенными автономными миссиями для групп дронов.

## Макро-карта применений дронов и роев

### 1. Наблюдение и разведка среды

- картография;
- мониторинг пожаров;
- поиск людей;
- контроль границ и периметров;
- наблюдение за животными, посевами, лесами;
- промышленная инспекция;
- контроль строительных объектов;
- ситуационная осведомленность в зонах ЧС.

### 2. Покрытие пространства

- обследовать большую область;
- покрыть территорию сеткой датчиков;
- держать постоянное наблюдение;
- патрулировать;
- обновлять карту с заданной частотой;
- мониторить динамическую границу события: пожар, наводнение, загрязнение.

### 3. Инспекция объектов

- линии электропередач;
- трубопроводы;
- солнечные фермы;
- мосты;
- шахты;
- склады;
- порты;
- стройки;
- аварийные зоны;
- промышленное оборудование.

### 4. Связь и инфраструктура

- временная mesh-сеть;
- воздушные ретрансляторы;
- emergency communications;
- связь для наземных роботов;
- сеть над зоной катастрофы;
- связь в горах, лесах, пустыне;
- восстановление связи после разрушения наземной инфраструктуры.

### 5. Логистика

- доставка;
- распределение малых грузов;
- медицинская доставка;
- снабжение удаленных точек;
- инвентаризация складов;
- перемещение сенсоров;
- работа с мобильными базами.

### 6. Работа с наземными и морскими системами

- дроны + UGV;
- дроны + USV;
- дроны + стационарные сенсоры;
- дроны + базовые станции;
- дроны как разведчики для наземных роботов;
- дроны как ретрансляторы для наземных групп.

### 7. Аварийные и опасные среды

- пожар;
- химические утечки;
- радиация;
- обрушения;
- наводнения;
- зоны без GNSS;
- зоны без связи;
- зоны с плохой видимостью;
- зоны, опасные для человека.

### 8. Шоу и массовая координация

- light shows;
- формации;
- синхронные траектории;
- массовое безопасное управление;
- проверка separation, geofence и failsafe на большом числе аппаратов.

### 9. Оборонные и security-сценарии

- наблюдение;
- охрана периметра;
- обнаружение нарушителей;
- counter-UAS;
- устойчивость к подавлению связи;
- работа в условиях деградации навигации и связи.

Примечание: такие сценарии разумно рассматривать на уровне архитектуры, detection, safety, resilience, связи и моделирования, не уходя в прикладную боевую тактику.

## Архитектурные оси

### 1. По централизации

- centralized fleet: все решает центр;
- decentralized swarm: агенты договариваются локально;
- hierarchical: лидеры групп, подгруппы, роли;
- hybrid: стратегию задает центр, тактику решают локальные группы.

### 2. По связности

- все видят всех;
- локальная mesh-сеть;
- intermittently connected;
- delay-tolerant network;
- связь через ретрансляторы;
- связь через наземную станцию;
- связь через LTE/5G/satellite;
- полная автономия при потере связи.

### 3. По составу

- гомогенный рой: все аппараты одинаковые;
- гетерогенный рой: разные типы аппаратов;
- mixed fleet: дроны, UGV, стационарные сенсоры, операторы;
- expendable + high-value nodes;
- роли: scouts, relays, mappers, inspectors, carriers.

### 4. По уровню участия оператора

- operator-in-the-loop;
- operator-on-the-loop;
- supervised autonomy;
- mission-level autonomy;
- fully autonomous degraded mode.

### 5. По среде

- GNSS available;
- GNSS-denied;
- indoor;
- urban canyon;
- forest;
- mountain;
- maritime;
- disaster zone;
- contested communications;
- плохая видимость;
- динамическая опасная зона.

### 6. По типу миссии

- static plan;
- dynamic replanning;
- event-driven;
- persistent surveillance;
- search under uncertainty;
- cooperative mapping;
- relay positioning;
- multi-object tracking;
- periodic inspection;
- infrastructure recovery.

## Слои серьезной системы

```text
Mission Layer
  цели, ограничения, приоритеты, правила, область операции

Planning Layer
  декомпозиция миссии, маршруты, задачи, расписание

Fleet Layer
  аппараты, роли, статусы, батареи, payloads, health, lifecycle

Swarm / Coordination Layer
  распределение задач, consensus, локальная координация, связность

Comms Layer
  mesh, latency, packet loss, routing, bandwidth, QoS

World Model Layer
  карта, объекты, зоны, uncertainty, shared state, sensor fusion

Safety Layer
  geofence, separation, collision avoidance, failsafe, return-to-home

Vehicle Interface Layer
  MAVLink, PX4/ArduPilot, ROS 2, payload control

Simulation / Digital Twin
  сценарии, физика, связь, сенсоры, отказы, replay, metrics
```

## Крупный объект разработки

Не "swarm simulator", а:

> Distributed Autonomous Mission System for Drone Fleets and Swarms.

Или:

> Mission-Level Digital Twin for Autonomous Drone Fleet/Swarm Systems.

Ключевые подсистемы:

- Mission DSL: описание миссий, зон, ролей, ограничений;
- Fleet model: аппараты, батарея, payload, скорость, сенсоры, состояние;
- Comms model: топология, packet loss, bandwidth, задержки;
- Task allocation engine: централизованное и распределенное назначение задач;
- World model: карта, зоны интереса, uncertainty;
- Safety engine: separation, geofence, no-fly zones, failsafe;
- Scenario runner: воспроизводимые сценарии;
- Metrics engine: качество выполнения, стоимость, надежность, связь;
- Visualization: карта, состояние флота, timeline, replay;
- Backend adapters: сначала internal sim, позже PX4/ArduPilot SITL.

## Reference missions

Архитектуру лучше проектировать не вокруг абстрактных агентов, а вокруг нескольких эталонных миссий.

### 1. Search and Rescue

Условия:

- большая зона поиска;
- неизвестное положение целей;
- ограниченная связь;
- разные сенсоры;
- неизвестная или частично известная карта;
- отказы аппаратов;
- приоритет времени.

Что проверять:

- распределение зон;
- вероятность обнаружения;
- стратегия поиска;
- роль relay-дронов;
- перепланирование при отказе;
- работа со stale data.

### 2. Wildfire Mapping

Условия:

- фронт пожара движется;
- часть зоны опасна;
- нужна периодическая актуализация карты;
- дроны с thermal payload;
- связь может деградировать;
- базовая станция может видеть не всех.

Что проверять:

- обновление карты во времени;
- баланс "наблюдать фронт" vs "исследовать новую область";
- безопасность относительно опасной зоны;
- relay positioning;
- потеря аппаратов.

### 3. Infrastructure Inspection

Условия:

- ЛЭП, трубопровод, солнечная ферма, мост или промышленный объект;
- маршрутная инспекция;
- high-resolution sensing;
- battery constraints;
- требование повторяемости и полного покрытия.

Что проверять:

- распределение участков;
- минимизация времени;
- качество покрытия;
- повторяемость;
- реакция на обнаруженные дефекты;
- возврат на базу при нехватке батареи.

### 4. Emergency Mesh Network

Условия:

- зона катастрофы;
- наземная связь разрушена;
- есть база;
- есть наземные точки, которым нужна связь;
- дроны должны занять позиции ретрансляторов;
- наземные узлы могут двигаться;
- связь нестабильна;
- батарея ограничена.

Что проверять:

- network availability;
- connectivity over time;
- relay placement;
- bandwidth allocation;
- перемещение relay-дронов;
- деградация при packet loss;
- восстановление после отказов.

### 5. Disaster Area Search and Communications Recovery

Особенно сильный первый крупный сценарий, потому что объединяет:

- fleet management;
- swarm coordination;
- mesh communications;
- task allocation;
- safety;
- mission planning;
- uncertainty.

Пример:

- зона катастрофы;
- разрушена связь;
- есть база;
- есть наземные точки, которым нужна связь;
- часть дронов scout;
- часть relay;
- часть mapping;
- цели появляются динамически;
- связь нестабильна;
- батарея ограничена;
- часть аппаратов отказывает.

## Как планировать сверху

### 1. Сделать карту домена

- типы миссий;
- типы агентов;
- типы payload;
- типы связи;
- типы отказов;
- типы сред;
- типы метрик.

### 2. Выбрать reference architecture

- какие слои есть;
- какие данные между ними ходят;
- что централизовано;
- что распределено;
- где граница симуляции и реального автопилота;
- где граница mission autonomy и vehicle control.

### 3. Описать модель мира

- карта;
- зоны;
- цели;
- угрозы/опасности;
- uncertainty;
- динамические события;
- препятствия;
- no-fly zones.

### 4. Описать модель флота

- vehicle capabilities;
- sensors;
- communication devices;
- battery;
- energy model;
- failure modes;
- roles;
- payloads;
- constraints.

### 5. Описать миссионный язык

Пример:

```yaml
mission:
  type: search_and_rescue
  area: mountain_sector_7
  objectives:
    - maximize_probability_of_detection
    - maintain_comms_to_base
    - avoid_no_fly_zones
  constraints:
    min_separation_m: 20
    max_mission_time_min: 45
    require_return_battery_percent: 20

fleet:
  vehicles:
    - count: 6
      role: scout
      sensor: thermal
    - count: 2
      role: relay
      comms: high_bandwidth
```

### 6. Потом выбирать алгоритмы

- для search: coverage, frontier, information gain;
- для relay: connectivity optimization;
- для task allocation: auction, CBBA, Hungarian;
- для safety: ORCA, velocity obstacles, barrier functions;
- для coordination: centralized, hierarchical, decentralized;
- для communications: gossip, mesh routing, delay-tolerant strategies.

## Как моделировать без лаборатории и дронов

Главная идея: не пытаться симулировать "дрон целиком" с первого дня. Серьезная система моделируется многоуровнево, с разной точностью на разных слоях.

Физические дроны понадобятся позже для валидации нижнего уровня, но верхний и средний уровни можно разрабатывать в симуляции:

- mission autonomy;
- distributed task allocation;
- resilient communications;
- fleet/swarm coordination;
- failure handling;
- scenario evaluation;
- metrics and replay.

## Уровни симуляции

### Уровень A: Mission Simulation

Дрон - это не физическое тело с моторами, а ресурс с возможностями:

```text
vehicle:
  position
  battery
  speed_limit
  sensor_range
  comms_range
  payload
  role
  health
  current_task
```

Миссия:

```text
area
objectives
constraints
vehicles
tasks
events
failures
metrics
```

На этом уровне моделируются:

- поиск;
- покрытие территории;
- инспекция;
- ретрансляция связи;
- распределение ролей;
- отказы;
- потеря связи;
- перепланирование.

Это главный уровень для макро-проекта.

### Уровень B: Kinematic Simulation

Дрон движется как точка или простое тело:

```text
position += velocity * dt
velocity ограничена max_speed
acceleration ограничена max_accel
turn_rate ограничен
```

Этого достаточно для:

- маршрутов;
- столкновений;
- separation;
- покрытия;
- связи;
- приближенного времени миссии;
- оценки батареи.

На этом уровне не нужны моторы, пропеллеры, PID и аэродинамика.

### Уровень C: Communication Simulation

Отдельно моделируется сеть:

```text
link exists if distance < range
packet_loss = function(distance, obstacles, congestion)
latency = base + jitter + queue_delay
bandwidth = limited
messages may be dropped/reordered/delayed
```

Можно проверять:

- mesh-связь;
- relay-дроны;
- потерю базовой станции;
- delayed state;
- stale data;
- gossip;
- consensus;
- distributed task allocation.

Для роя и флота связь часто важнее физики полета.

### Уровень D: Sensor / World Model Simulation

Сенсоры моделируются не как реальные камеры, а как вероятностные источники данных:

```text
thermal_sensor:
  range: 120m
  false_positive_rate: 0.02
  false_negative_rate: 0.15
  detection_probability depends on distance/weather/occlusion
```

Можно моделировать:

- обнаружение целей;
- uncertainty map;
- обновление карты;
- false positives;
- missed detections;
- качество покрытия;
- sensor fusion.

Photorealistic camera simulation для mission-level исследований не обязательна.

### Уровень E: Vehicle / Autopilot Simulation

Поздний уровень:

- PX4 SITL;
- ArduPilot SITL;
- Gazebo;
- Webots;
- Isaac Sim;
- MAVLink;
- ROS 2.

Этот уровень нужен, когда требуется проверить, что команды совместимы с реальным автопилотом. Он не должен быть центром проекта на старте.

## Что можно изучать без физических дронов

Пример сценария Search and Rescue:

```text
Есть область 10 x 10 км.
Есть 8 дронов.
2 из них с тепловизорами дальнего радиуса.
2 могут быть relay.
4 обычные scout.
Связь с базой доступна только в части области.
Цель неизвестна.
Погода ухудшается.
Один дрон теряется через 20 минут.
```

Исследовательские вопросы:

- как распределить зоны;
- кого назначить ретранслятором;
- когда отправлять дрон на базу;
- как перестроиться при отказе;
- стоит ли держать связность всегда или разрешить автономные выходы;
- какая стратегия быстрее находит цель;
- сколько сообщений нужно;
- что ломается при packet loss 30%;
- какая роль важнее: больше scouts или больше relays.

## Архитектура симулятора

```text
+----------------------+
| Scenario Definition  |
| YAML / JSON / RON    |
+----------+-----------+
           |
           v
+----------------------+
| Simulation Runtime   |
| clock, events, seed  |
+----------+-----------+
           |
           v
+----------------------+
| World Model          |
| map, zones, targets  |
+----------+-----------+
           |
           v
+----------------------+
| Fleet Model          |
| vehicles, roles      |
+----------+-----------+
           |
           v
+----------------------+
| Comms Model          |
| links, loss, latency |
+----------+-----------+
           |
           v
+----------------------+
| Mission Logic        |
| planners, allocators |
+----------+-----------+
           |
           v
+----------------------+
| Metrics / Replay     |
| logs, traces, plots  |
+----------------------+
```

Симулятор должен быть:

- event/time-step based;
- воспроизводимый;
- seed-based;
- headless-first;
- с возможностью replay;
- с отделением модели от визуализации.

## Метрики

Оценивать надо не то, "красиво ли летит", а:

- mission success rate;
- time to complete;
- probability of detection;
- coverage over time;
- min separation;
- number of collisions / near misses;
- battery margin;
- communication load;
- network partition time;
- task reassignment time;
- performance under failures;
- centralized vs decentralized comparison;
- effect of stale information;
- value of relay drones;
- sensitivity to sensor quality;
- robustness to losing 10%, 20%, 30% agents.

## Как компенсировать отсутствие физики

На mission-level достаточно задать operational envelope:

```text
max_speed
max_acceleration
turn_radius
battery_capacity
energy_per_meter
hover_cost
climb_cost
payload_weight_penalty
wind_penalty
```

Затем делать sensitivity analysis:

```text
А что если скорость на 20% меньше?
А что если батарея хуже на 30%?
А что если ветер увеличивает расход в 1.5 раза?
А что если один тип дронов в два раза медленнее?
```

Это полезнее, чем пытаться идеально симулировать моторы на раннем этапе.

## Минимальный серьезный стек

Под текущий профиль:

```text
Rust core
serde для сценариев
petgraph для сетей/графов
nalgebra или glam для математики
tracing для логов
criterion для benchmark
Bevy или egui для визуализации
```

Дополнительно:

```text
Python notebooks или Polars/DataFusion для анализа результатов
PX4/ArduPilot SITL как optional backend
ROS 2 только если реально понадобится интеграция
```

Главное:

- визуализация не должна быть главным продуктом;
- главный продукт - headless simulation, метрики, сценарии и replay.

## Главная развилка специализации

Нужно выбрать не "рой или флот", а тип системной сложности.

### 1. Mission autonomy

Самое широкое направление.

Вопрос:

> Как группа аппаратов выполняет миссию от цели до результата?

### 2. Resilient communications

Наиболее близко к системному/embedded профилю.

Вопрос:

> Как система работает при потере связи, задержках, packet loss, сетевых разделениях и ограниченной пропускной способности?

### 3. Distributed task allocation

Самое алгоритмическое направление.

Вопрос:

> Кто что делает, как перераспределять задачи, как жить без надежного центра?

Рекомендуемая формулировка:

> Mission autonomy через призму resilient communications and distributed task allocation.

Большая рамка: автономные миссии.

Специализация внутри: связь, отказы, распределенность.

## Что считать настоящим результатом

Не "в окне летают точки", а:

```text
Запуск 1000 сценариев с разными seed.

Сравнение 4 стратегий:
  1. centralized planner
  2. greedy decentralized
  3. auction-based allocation
  4. relay-aware strategy

Отчет:
  success rate
  mean mission time
  coverage %
  network availability
  messages per minute
  battery margin
  degradation under packet loss
```

Это уже исследовательская платформа, а не песочница.

## Новый верхнеуровневый план

1. Написать domain architecture document.
2. Выбрать 3-4 reference missions.
3. Спроектировать mission/fleet/world/comms/safety model.
4. Сделать формат сценариев.
5. Сделать симуляционный runtime.
6. Сделать несколько стратегий управления.
7. Сравнивать стратегии по метрикам.
8. Потом подключать SITL как один из backend'ов.

## Короткий итог

Без мастерской и дронов можно заниматься этой темой серьезно.

Но моделировать надо не пропеллеры и не полет как самоцель, а:

```text
mission autonomy
distributed task allocation
resilient communications
fleet/swarm coordination
failure handling
scenario evaluation
metrics and replay
```

Физические дроны нужны позже для валидации нижнего уровня. Верхний и средний уровни системы можно глубоко разрабатывать в симуляции.

Главная формулировка проекта:

> Mission-level digital twin for autonomous drone fleet/swarm systems.

По-русски:

> Цифровой двойник автономных миссий для групп дронов: миссии, флот, связь, отказы, распределение задач, безопасность, метрики и replay.
