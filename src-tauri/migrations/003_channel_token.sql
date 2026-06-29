-- Phase 6 follow-up: persist the extension pairing token so it survives app
-- restarts (so the extension is paired once, not re-paired every launch).
--
-- The token is a capability for the loopback channel, NOT a vault secret: it
-- only grants access while the vault is unlocked and never decrypts anything
-- (THREAT F14). Storing it in the clear is consistent with the extension
-- persisting it in the clear.

ALTER TABLE vault ADD COLUMN channel_token TEXT;
