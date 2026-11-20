# Monitor empty channels (2026-11-20 00:41)

Fixed the Monitor (Live) view showing a near-empty "Throttle" readout card and a completely
blank chart box underneath the Torque chart whenever the throttle channel had no live data yet
(disarmed, no driver chosen, or `manual` driver, where the backend only emits throttle samples
once the actuator is armed). The card showed a bare "—" and the chart rendered no axes, no grid
and no legend at all, since `LineChart`/`uPlot` had an empty x series to draw.

- **Root cause**: `LiveView` rendered a `Readout` and a `LiveChart` for every enabled channel
  reported by the device, regardless of whether `liveBuffer` had ever received a sample for it.
  The throttle channel is reported by the device at all times but only carries real samples
  while armed (see `mockBackend.ts`'s `tick()`), so most of the time it is a channel with no
  data rather than a channel with a zero value.
- **Fix**: `LiveView` now re-filters `channels` on a 250 ms tick, keeping only the ones with
  `liveBuffer.recent(id) !== null`, and renders readouts/charts from that filtered list. A
  channel's card and chart now appear as soon as it starts streaming and drop out again if it
  stops, instead of sitting there empty. This is channel-agnostic (not special-cased to
  throttle), so the same fix applies to any future sensor that is reported but not always live.
- **Verification**: ran the Vite dev server against `mockBackend` under a scratch Playwright
  install and compared screenshots before/after in both the no-driver-chosen and `manual`-driver
  states (the two states a thrust/torque-only bench sits in most of the time): the Throttle card
  and its chart no longer appear until the channel actually has data. `tsc --noEmit` passes.
