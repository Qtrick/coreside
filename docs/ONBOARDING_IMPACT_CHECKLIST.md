# Onboarding impact checklist

- [ ] Migration `017` applies on upgrade; progress table present
- [ ] First empty profile shows welcome once (main window)
- [ ] Escape / Explore skips without blocking chat
- [ ] Tour spotlights composer, app panel, help nav; missing target → centered popover
- [ ] `prefers-reduced-motion` disables spotlight transition
- [ ] Help & learning restart / continue work after skip
- [ ] `CORESIDE_DISABLE_ONBOARDING=1` and E2E hide welcome
- [ ] Secondary tool window does not show welcome
- [ ] Sample seed/cleanup does not delete user chats/apps
- [ ] Consumer UI: Apps labels; Search setup hides `.env` / npm without developer mode
- [ ] Wallpapers under Appearance; search “wallpaper” → Appearance

Evidence: `npm run test:onboarding`, `cargo test --lib db::tutorial::`, `npm run test:settings`. Not Desktop Verified by itself.
