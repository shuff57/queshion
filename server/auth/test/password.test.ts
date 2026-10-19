import { describe, expect, it } from "vitest";
import { checkPasswordPolicy, hashPassword, parsePhc, verifyPassword } from "../src/index.js";
import { StubHasher } from "./helpers.js";

const E = "jordan.smith@school.example.org";
const check = (pw: string) => checkPasswordPolicy(pw, E, 10, new Set());

describe("password policy", () => {
  it("accepts ordinary passphrases", () => {
    for (const pw of ["violet-harbor-tractor", "Tr0ub4dor&3-horse", "correct horse battery", "日本語のパスワードです"]) expect(check(pw)).toBeNull();
  });

  it("counts characters, not UTF-16 units", () => {
    expect(check("😀😀😀😀😀😀😀😀😀")).toBe("too_short"); // 9 emoji = 18 units
    expect(check("😀😀😀🙂🙂🙂😎😎😎🤖")).toBeNull();
  });

  it("rejects common passwords, padded common words and runs", () => {
    for (const pw of ["1234567890", "PASSWORD123", "Password!!!!", "welcome2026", "qwerty123456", "abcdefghij", "9876543210", "zzzzzzzzzzzz", "P@ssw0rd!!!", "Admin2026!!", "!!!password!!!"])
      expect(check(pw), pw).toBe("common");
  });

  it("rejects passwords built on the email", () => {
    expect(check("jordan.smith2026")).toBe("contains_email");
    expect(check(E)).toBe("contains_email");
  });

  it("enforces length bounds and an extra word list", () => {
    expect(check("x".repeat(9))).toBe("too_short");
    expect(check("ab".repeat(257))).not.toBeNull();
    expect(checkPasswordPolicy("pleasantvalley", E, 10, new Set(["pleasantvalley"]))).toBe("common");
  });
});

describe("PHC encoding", () => {
  it("round-trips and verifies in constant time", async () => {
    const h = new StubHasher();
    const stored = await hashPassword(h, "some-long-password", { m: 19456, t: 2, p: 1 });
    expect(stored.phc).toMatch(/^\$argon2id\$v=19\$m=19456,t=2,p=1\$[A-Za-z0-9+/]{22}\$[A-Za-z0-9+/]{43}$/);
    expect(parsePhc(stored.phc)?.params).toEqual({ m: 19456, t: 2, p: 1 });
    expect(await verifyPassword(h, stored.phc, "some-long-password")).toBe(true);
    expect(await verifyPassword(h, stored.phc, "some-long-passwore")).toBe(false);
    expect(await verifyPassword(h, "$argon2i$v=19$m=1,t=1,p=1$AAAA$AAAA", "x")).toBe(false);
    expect(await verifyPassword(h, "garbage", "x")).toBe(false);
  });

  it("refuses a hasher that returns the wrong tag length", async () => {
    await expect(hashPassword({ argon2id: () => new Uint8Array(16) }, "pw-pw-pw-pw", { m: 8, t: 1, p: 1 })).rejects.toThrow();
  });
});
