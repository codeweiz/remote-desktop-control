import { describe, it, expect } from 'vitest';
import { generateToken, validateToken } from './auth.js';

describe('auth', () => {
  it('generates a 32-char hex token', () => {
    const token = generateToken();
    expect(token).toMatch(/^[a-f0-9]{32}$/);
  });

  it('validates correct token', () => {
    const token = generateToken();
    expect(validateToken(token, token)).toBe(true);
  });

  it('rejects incorrect token', () => {
    const token = generateToken();
    expect(validateToken('wrong', token)).toBe(false);
  });

  it('rejects empty token', () => {
    const token = generateToken();
    expect(validateToken('', token)).toBe(false);
  });
});
