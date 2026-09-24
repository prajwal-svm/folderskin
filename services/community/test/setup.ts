import { applyD1Migrations } from "cloudflare:test";
import { env } from "cloudflare:workers";

// Every test file starts from the migrations in migrations/, applied once.
await applyD1Migrations(env.DB, env.TEST_MIGRATIONS);
