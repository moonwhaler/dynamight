export type StrengthLevel = 'weak' | 'fair' | 'good' | 'strong';

export interface PasswordStrength {
  score: number; // 0-100
  level: StrengthLevel;
  label: string;
}

export function calculatePasswordStrength(password: string): PasswordStrength {
  if (!password) {
    return { score: 0, level: 'weak', label: 'Too short' };
  }

  let score = 0;

  // Length scoring (up to 30 points)
  if (password.length >= 8) score += 10;
  if (password.length >= 12) score += 10;
  if (password.length >= 16) score += 10;

  // Character variety (up to 40 points)
  if (/[a-z]/.test(password)) score += 10; // lowercase
  if (/[A-Z]/.test(password)) score += 10; // uppercase
  if (/[0-9]/.test(password)) score += 10; // numbers
  if (/[^a-zA-Z0-9]/.test(password)) score += 10; // special chars

  // Complexity bonus (up to 30 points)
  const uniqueChars = new Set(password).size;
  if (uniqueChars >= 6) score += 10;
  if (uniqueChars >= 10) score += 10;
  if (uniqueChars >= 14) score += 10;

  // Penalties for common patterns
  if (/^[a-zA-Z]+$/.test(password)) score -= 10; // only letters
  if (/^[0-9]+$/.test(password)) score -= 20; // only numbers
  if (/(.)\1{2,}/.test(password)) score -= 10; // repeated chars (aaa, 111)
  if (/^(123|abc|qwe|password|admin)/i.test(password)) score -= 20; // common starts

  // Clamp score
  score = Math.max(0, Math.min(100, score));

  // Determine level and label
  if (password.length < 8) {
    return { score: Math.min(score, 25), level: 'weak', label: 'Too short' };
  }

  if (score < 30) {
    return { score, level: 'weak', label: 'Weak' };
  } else if (score < 50) {
    return { score, level: 'fair', label: 'Fair' };
  } else if (score < 75) {
    return { score, level: 'good', label: 'Good' };
  } else {
    return { score, level: 'strong', label: 'Strong' };
  }
}
