"""Rate-limit constants (dispatchers own Crawl4AI RateLimiter wiring)."""

BASE_DELAY_SECONDS = (2.0, 5.0)
MAX_BACKOFF_SECONDS = 60.0
MAX_RETRIES = 2
