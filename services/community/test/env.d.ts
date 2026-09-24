import type { D1Migration } from "cloudflare:test";
import type { Env as ServiceEnv } from "../src/env";

declare global {
  namespace Cloudflare {
    interface Env extends ServiceEnv {
      TEST_MIGRATIONS: D1Migration[];
    }
  }
}
