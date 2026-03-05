import { randomBytes } from 'node:crypto';

export function generateToken(): string {
  return randomBytes(16).toString('hex');
}

export function validateToken(provided: string, expected: string): boolean {
  if (!provided || !expected) return false;
  return provided === expected;
}
