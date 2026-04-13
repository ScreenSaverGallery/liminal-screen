// Liminal Screen API Security Module
// Provides shared secret authentication for secure remote options integration

/**
 * Security configuration for Liminal API
 */
export interface SecurityConfig {
  /** Shared secret key for authentication */
  sharedSecret?: string;

  /** Whether to require authentication */
  requireAuth?: boolean;

  /** Session timeout in milliseconds */
  sessionTimeout?: number;
}

/**
 * Authentication token structure
 */
export interface AuthToken {
  /** Timestamp when token was issued */
  timestamp: number;

  /** Random nonce for replay protection */
  nonce: string;

  /** HMAC signature */
  signature: string;
}

/**
 * Security manager for Liminal API
 */
export class SecurityManager {
  private config: Required<SecurityConfig>;
  private activeSessions: Map<string, number> = new Map(); // token -> expiry timestamp

  constructor(config?: SecurityConfig) {
    this.config = {
      sharedSecret:
        config?.sharedSecret || process.env.LIMINAL_API_SECRET || "",
      requireAuth: config?.requireAuth ?? false,
      sessionTimeout: config?.sessionTimeout || 3600000, // 1 hour default
    };
  }

  /**
   * Generate an authentication token
   */
  generateAuthToken(): AuthToken | null {
    if (!this.config.sharedSecret || !this.config.requireAuth) {
      return null;
    }

    const timestamp = Date.now();
    const nonce = this.generateNonce();
    const data = `${timestamp}:${nonce}`;
    const signature = this.createSignature(data);

    return {
      timestamp,
      nonce,
      signature,
    };
  }

  /**
   * Validate an authentication token
   */
  validateAuthToken(token: AuthToken): boolean {
    if (!this.config.requireAuth) {
      return true; // Authentication not required
    }

    if (!this.config.sharedSecret) {
      console.warn("Authentication required but no shared secret configured");
      return false;
    }

    // Check timestamp (prevent replay attacks)
    const now = Date.now();
    if (now - token.timestamp > this.config.sessionTimeout) {
      console.warn("Authentication token expired");
      return false;
    }

    // Verify signature
    const data = `${token.timestamp}:${token.nonce}`;
    const expectedSignature = this.createSignature(data);

    if (token.signature !== expectedSignature) {
      console.warn("Invalid authentication signature");
      return false;
    }

    // Check for replay (store used nonces temporarily)
    const tokenKey = `${token.timestamp}:${token.nonce}`;
    if (this.activeSessions.has(tokenKey)) {
      console.warn("Replay attack detected");
      return false;
    }

    // Store session
    this.activeSessions.set(tokenKey, now + this.config.sessionTimeout);
    this.cleanupExpiredSessions();

    return true;
  }

  /**
   * Create HMAC signature for data
   */
  private createSignature(data: string): string {
    if (!this.config.sharedSecret) {
      throw new Error("No shared secret configured");
    }

    // Simple HMAC-like implementation (in production, use crypto.subtle)
    const encoder = new TextEncoder();
    const dataBytes = encoder.encode(data);
    const secretBytes = encoder.encode(this.config.sharedSecret);

    // This is a simplified implementation - in production use proper crypto
    // For now, we'll use a basic hash-based approach for demonstration
    const combined = [...dataBytes, ...secretBytes];
    let hash = 0;
    for (let i = 0; i < combined.length; i++) {
      hash = ((hash << 5) - hash + combined[i]) | 0;
    }

    return hash.toString(16);
  }

  /**
   * Generate random nonce
   */
  private generateNonce(length: number = 16): string {
    const chars =
      "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let result = "";
    for (let i = 0; i < length; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
  }

  /**
   * Clean up expired sessions
   */
  private cleanupExpiredSessions(): void {
    const now = Date.now();
    for (const [key, expiry] of this.activeSessions.entries()) {
      if (now > expiry) {
        this.activeSessions.delete(key);
      }
    }
  }

  /**
   * Check if authentication is required
   */
  isAuthenticationRequired(): boolean {
    return this.config.requireAuth;
  }

  /**
   * Get security configuration status
   */
  getSecurityStatus(): {
    enabled: boolean;
    hasSecret: boolean;
    sessionTimeout: number;
  } {
    return {
      enabled: this.config.requireAuth,
      hasSecret: !!this.config.sharedSecret,
      sessionTimeout: this.config.sessionTimeout,
    };
  }
}

// Export singleton instance
export const securityManager = new SecurityManager();

// Types are already exported as interfaces, no need to export type again
