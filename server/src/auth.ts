import { timingSafeEqual } from "node:crypto";

/**
 * Checks a Bearer token against the configured named tokens.
 * Returns the token's name on success, null otherwise.
 */
export function authenticate(
  authorizationHeader: string | undefined,
  tokens: Map<string, string>
): string | null {
  if (!authorizationHeader?.startsWith("Bearer ")) return null;
  const presented = Buffer.from(authorizationHeader.slice(7).trim());
  let matchedName: string | null = null;
  // Always compare against every token so timing does not leak which entry matched.
  for (const [name, token] of tokens) {
    const expected = Buffer.from(token);
    if (presented.length === expected.length && timingSafeEqual(presented, expected)) {
      matchedName = name;
    }
  }
  return matchedName;
}
