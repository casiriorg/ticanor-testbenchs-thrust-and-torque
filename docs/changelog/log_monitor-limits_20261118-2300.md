# Monitor limits (2026-11-18 23:00)

Renamed the Live view to "Monitor" and added configured safety/maximum reference lines for load cell channels.

- **Nav label**: "Live" → "Monitor" tab (en/es), no internal identifiers renamed.
- **Load cell profile data**: added `safe_max_kgf` to each entry in `load_cell_profiles`
  (`crates/tb-core/src/config/defaults.json` + `defaults.rs` struct + `src/services/types.ts`), the
  manufacturer's rated overload ceiling for that cell model (e.g. 10 kg cell gets 10.5 kgf), distinct from
  `capacity_kg`.
- **Channel ↔ profile link**: added `channel_load_cell` (channel id → profile id) to `AppSettings`
  (`settings.rs`, `defaults.json`, `types.ts`), persisted like `channel_names`. Set from the Device view
  whenever "Apply profile" is used; the profile selector now defaults to the saved choice instead of always
  the first profile.
- **Monitor display**: each load-cell channel's readout now shows, when configured, the operator's safety
  cut-off (`Settings → safety.max_thrust_n` / `max_torque_nm`) and the assigned cell's `safe_max_kgf`,
  converted to the channel's display unit. Both are also drawn as dashed reference lines on that channel's
  live chart (`channelLimitsSI` helper in `utils/units.ts`).
- **LineChart**: `Line` gained optional `dash`/`width` so reference lines can be styled differently from the
  data series.
- **Mock backend**: demo settings/profiles updated to exercise the new fields in `npm run dev`.
