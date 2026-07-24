export type AuthenticatedUser = {
  id: string;
  role: string;
};

export interface UserAuthenticator {
  authenticate(token: string): Promise<AuthenticatedUser | null>;
}

type Fetcher = (
  input: string | URL | Request,
  init?: RequestInit,
) => Promise<Response>;

export class SupabaseUserAuthenticator implements UserAuthenticator {
  constructor(
    private readonly url: string,
    private readonly anonKey: string,
    private readonly fetcher: Fetcher = fetch,
  ) {}

  async authenticate(token: string): Promise<AuthenticatedUser | null> {
    if (!this.url || !this.anonKey) throw new Error("INTERNAL_ERROR");
    if (token === this.anonKey) return null;

    let response: Response;
    try {
      response = await this.fetcher(`${this.url}/auth/v1/user`, {
        headers: {
          apikey: this.anonKey,
          Authorization: `Bearer ${token}`,
        },
      });
    } catch {
      return null;
    }
    if (!response.ok) return null;

    let value: unknown;
    try {
      value = await response.json();
    } catch {
      return null;
    }
    if (!value || typeof value !== "object") return null;
    const user = value as Record<string, unknown>;
    if (
      typeof user.id !== "string" ||
      user.id.length === 0 ||
      typeof user.role !== "string" ||
      user.role === "anon" ||
      user.role === "anonymous"
    ) {
      return null;
    }
    return { id: user.id, role: user.role };
  }
}
