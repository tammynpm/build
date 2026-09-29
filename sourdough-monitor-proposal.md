# Sourdough Monitor — Project Proposal

# Tammy Nguyen Sep 29th, 2026
# written with help from Claude

## Project overview
An ESP32-based device that detects when a sourdough starter needs feeding (via rise, temperature, weight, and gas sensors) and auto-stirs it with a repurposed-tool stirring rig. Deliverable: a working sensor + stirrer rig with logged data proving it flags the peak across one full feed cycle.

## Rationale
I own a starter and keep misjudging when to feed it. This project gives me practice fusing multiple cheap sensors into one reliable signal, plus a real mechanical challenge: standard magnetic stirrers can't move dough this thick.

## Proposed methodology
Phase 1: wire an ESP32 to an HC-SR04/VL53L0X (rise), DHT11 (temp/humidity), load cell + HX711 (weight), and MQ-135 (gas trend); log all four over a feed cycle and combine into a "needs feeding" rule triggering a buzzer/LED. Phase 2: build a stirring arm from a repurposed tool head (e.g., the detached head of a small spatula) mounted directly onto the shaft of a 28BYJ-48 stepper motor + ULN2003 driver via a small shaft coupler (chosen over a DC gear motor for its higher torque at low, controllable speed), sized/shaped by trial and error against the jar radius and prototyped first on a flour-water paste before touching the live starter. Sensing is paused during stir cycles to avoid motor vibration/EMI noise.

**Trigger logic**: the stir fires once, immediately after a feed event — not on a timer and not tied to the "needs feeding" detection, which is a separate alert-to-user signal. This matches sourdough practice: the main reason to mix right after feeding is to evenly hydrate the new flour/water and introduce oxygen, which briefly boosts early yeast growth before fermentation turns anaerobic. v1 detects the feed event via a manual button press ("I just fed it"); a stretch goal is auto-detecting it from a sudden load-cell weight increase. Repeated or continuous stirring during the fermentation window has no real benefit and is avoided.

## Anticipated challenges
Stirring-arm design is untested and may need iteration; the motor drives the arm directly (shaft mounted, no magnetic coupling), so keeping the motor/electronics isolated from the starter and moisture is a real food-safety and durability concern to solve; motor vibration/EMI can corrupt load cell and ultrasonic readings; MQ-135 drifts with humidity and needs a warm-up/recalibration; DHT11 is slower and less precise than a DS18B20 probe and isn't waterproof, so it needs to sit near (not in) the starter.

## Bill of Materials
| Part | Cost |
|---|---|
| ESP32 dev board | $7 |
| HC-SR04 / VL53L0X | $3 |
| DHT11 + wiring | $2 |
| Load cell + HX711 | $5 |
| MQ-135 | $3 |
| 28BYJ-48 stepper + ULN2003 driver | $3 |
| Shaft coupler + spatula head (repurposed) | $2 |
| Buzzer/LED, misc electronics | $8 |
| Power/enclosure misc | $8 |
| **Total** | **~$41** (budget: $60) |

## Research / Inspiration
Chemistry-lab stir plates inspired the general concept of a motor-driven stirrer, but adapted to direct drive with a repurposed spatula-style head since dough is too viscous and stiff for a plain magnetic stir bar to move. DIY fermentation-monitoring builds (load cell/ultrasonic rise tracking) informed the sensor approach.
