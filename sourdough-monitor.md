# Sourdough Monitor — Project Plan

## Goal
A cheap DIY electronics project (learning-focused) that:
1. Detects when the sourdough starter needs feeding.
2. Automatically stirs the starter with a magnetic stirring mechanism.
3. Stays as low-cost as possible (~$60 target).

## Detection strategy
Combine four low-cost signals rather than relying on one, since each alone is a rough proxy for yeast activity:

| Signal | Sensor | Approx. cost | What it tells you |
|---|---|---|---|
| Rise height | HC-SR04 ultrasonic (or VL53L0X ToF) mounted above the jar | $2–5 | Dough rising then collapsing = past peak, time to feed |
| Temperature | DS18B20 waterproof probe | $2 | Feeds into expected rise time (warmer = faster fermentation) |
| Weight | Small load cell + HX711 amp under the jar | $4–6 | Secondary rise/activity signal, catches cases ultrasonic misses (uneven dough surface) |
| Fermentation gas | MQ-135 gas sensor | $2–3 | Rough relative CO2/VOC signal as activity proxy (not lab-accurate, but useful trend) |

Note: the MQ-135 is a cheap metal-oxide sensor, not a precision CO2 meter — treat its output as a relative trend (rising/falling), not an absolute ppm reading. A real NDIR CO2 sensor would be more accurate but costs $20+, blowing the budget for what it adds.

## Stirring mechanism
Magnetic auto-stirring cup approach:
- Small motor (N20 gear motor or repurposed PC fan motor) spins a magnet below/beside the jar.
- A PTFE-coated or food-safe stir bar (or DIY magnet in a sealed sleeve) sits in the starter and follows the rotating field.
- Driven via a transistor/MOSFET + PWM from the microcontroller so stir speed and duration are controllable, not just on/off.
- Triggered automatically after a feed event, or on a timer, rather than continuously (continuous stirring isn't necessary and wastes power/battery).

## Controller & output
- ESP32 dev board (WiFi lets you get a notification instead of only a local buzzer, and it's cheap — ~$6–8).
- Local feedback: buzzer or LED for "needs feeding now."
- Optional: push notification / simple web dashboard once core logic works (stretch goal, not required for v1).

## Rough budget (fits ~$60)
- ESP32 dev board: $7
- HC-SR04 or VL53L0X: $3
- DS18B20 + wiring: $2
- Load cell + HX711: $5
- MQ-135: $3
- N20 motor + driver transistor: $4
- Magnets + stir bar housing: $3
- Buzzer/LED, resistors, breadboard, jumper wires: $8
- Power supply/USB cable/enclosure misc: $8
- **Estimated total: ~$43**, leaving margin for mistakes/spares within the $60 budget.

## Phased plan

**Phase 1 — Core detection (build first, validate before adding motor)**
1. Wire ESP32 + DS18B20, log temperature over a full feed cycle to establish a baseline.
2. Add HC-SR04/ToF above a test jar, log rise/fall curve, confirm you can detect the peak.
3. Add load cell + HX711, compare its curve to the ultrasonic curve on the same feed cycle.
4. Add MQ-135, log its trend alongside the other three to see how well it correlates with the rise peak.
5. Combine into one "needs feeding" rule (e.g., rise has fallen X% from peak AND gas trend flattening/dropping) and trigger the buzzer/LED.

**Phase 2 — Magnetic stirrer**
1. Bench-test the motor + magnet spinning a stir bar in a jar of water/flour paste before touching the live starter.
2. Wire motor driver to ESP32, confirm PWM speed control works and doesn't interfere with sensor readings (motor noise/vibration can throw off load cell and ultrasonic readings — likely need to pause sensing while stirring).
3. Trigger stir cycle automatically after a feed event (manual button press to mark "I fed it" is simplest v1 trigger), run for a fixed duration.
4. Validate the starter doesn't get damaged (over-stirring can knock out gas/gluten structure) — start with short, gentle stir cycles and adjust.

**Phase 3 — Polish (optional stretch)**
1. Add WiFi push notification (e.g., via a simple webhook) instead of only local buzzer.
2. Log data over time (SD card or simple web endpoint) to see multi-day trends and refine the "needs feeding" rule.
3. 3D-print or hand-build a simple enclosure/stand for the sensors and motor.

## Open risks to watch
- Motor vibration/EMI may interfere with load cell and ultrasonic readings — plan to isolate mechanically or gate sensing around stir cycles.
- MQ-135 needs a warm-up period (minutes) and drifts with humidity — treat readings as relative, not absolute, and recalibrate baseline each session.
- Food-safety: anything touching the starter (stir bar, jar) should be food-safe/sealed; keep electronics physically isolated from the dough.
