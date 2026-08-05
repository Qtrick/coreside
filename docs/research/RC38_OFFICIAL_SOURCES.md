# RC3.8 research notes (access date: 2026-08-05)

Official sources consulted for Phase Zero / Phase One decisions:

- Stripe products and Prices: https://docs.stripe.com/products-prices/manage-prices
- Stripe Checkout Session create: https://docs.stripe.com/api/checkout/sessions/create
- Stripe subscription webhooks: https://docs.stripe.com/billing/subscriptions/webhooks
- Stripe webhook delivery/ordering: https://docs.stripe.com/webhooks
- Stripe customer portal sessions: https://docs.stripe.com/api/customer_portal/sessions/create
- Supabase Edge Function secrets: https://supabase.com/docs/guides/functions/secrets
- Supabase Edge Function auth: https://supabase.com/docs/guides/functions/auth
- WAI-ARIA modal dialog: https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/
- Apple App Review Guidelines: https://developer.apple.com/app-store/review/guidelines/
- OpenRouter model metadata: https://openrouter.ai/docs/guides/overview/models
- Ollama chat API: https://docs.ollama.com/api/chat
- HTML color input: https://html.spec.whatwg.org/multipage/input.html#color-state-(type=color)
- W3C Filter Effects: https://www.w3.org/TR/filter-effects-1/
- OWASP SSRF: https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html
- Rust `Ipv6Addr::to_ipv4_mapped` (stable since 1.63): https://doc.rust-lang.org/std/net/struct.Ipv6Addr.html

Implementation consequences recorded in code:

- Connect-time DNS pin via reqwest `resolve_to_addrs` after `classify_and_validate_endpoint`.
- Hosted gateway settles before `controller.close()`.
- Patch batches apply as one kernel transaction.
- MSRV raised to 1.85 to match declared platform classifiers and current toolchain.
- Billing credit ledger / pricing modal / promotion coordinator remain Phase Three+ work.
