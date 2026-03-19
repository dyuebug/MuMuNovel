const PLACEHOLDER_API_KEYS = new Set([
  'your_openai_api_key_here',
  'your_anthropic_api_key_here',
  'your_gemini_api_key_here',
  'your_api_key_here',
]);

const normalizeApiKey = (apiKey?: string | null): string =>
  typeof apiKey === 'string' ? apiKey.trim() : '';

export const isPlaceholderApiKey = (apiKey?: string | null): boolean => {
  const normalized = normalizeApiKey(apiKey);
  if (!normalized) {
    return false;
  }

  return PLACEHOLDER_API_KEYS.has(normalized.toLowerCase());
};

export const hasUsableApiCredentials = (
  apiKey?: string | null,
  apiBaseUrl?: string | null,
): boolean => {
  const normalizedApiKey = normalizeApiKey(apiKey);
  const normalizedBaseUrl = typeof apiBaseUrl === 'string' ? apiBaseUrl.trim() : '';

  return Boolean(
    normalizedApiKey &&
    normalizedBaseUrl &&
    !isPlaceholderApiKey(normalizedApiKey)
  );
};
