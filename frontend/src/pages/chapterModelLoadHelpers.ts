import { hasUsableApiCredentials } from '../utils/apiKey';

export type ModelOption = {
  value: string;
  label: string;
};

const normalizeOptionalSelectValue = (value: unknown): string | undefined => {
  if (typeof value !== 'string') {
    return undefined;
  }

  const normalizedValue = value.trim();
  return normalizedValue ? normalizedValue : undefined;
};

const normalizeModelOptions = (rawModels: unknown): ModelOption[] => {
  if (!Array.isArray(rawModels)) {
    return [];
  }

  const seenModelValues = new Set<string>();
  const normalizedModels: ModelOption[] = [];

  rawModels.forEach((rawModel) => {
    let nextValue: string | undefined;
    let nextLabel: string | undefined;

    if (typeof rawModel === 'string') {
      nextValue = normalizeOptionalSelectValue(rawModel);
      nextLabel = nextValue;
    } else if (rawModel && typeof rawModel === 'object') {
      const modelRecord = rawModel as Record<string, unknown>;
      nextValue = normalizeOptionalSelectValue(
        modelRecord.value ?? modelRecord.id ?? modelRecord.name ?? modelRecord.label,
      );
      nextLabel = normalizeOptionalSelectValue(
        modelRecord.label ?? modelRecord.name ?? modelRecord.value ?? modelRecord.id,
      );
    }

    if (!nextValue || seenModelValues.has(nextValue)) {
      return;
    }

    seenModelValues.add(nextValue);
    normalizedModels.push({
      value: nextValue,
      label: nextLabel ?? nextValue,
    });
  });

  return normalizedModels;
};

const areModelOptionsEqual = (leftOptions: ModelOption[], rightOptions: ModelOption[]): boolean => (
  leftOptions.length === rightOptions.length
  && leftOptions.every((option, index) => {
    const rightOption = rightOptions[index];
    return Boolean(rightOption)
      && option.value === rightOption.value
      && option.label === rightOption.label;
  })
);

export async function loadChapterAvailableModels({
  setAvailableModels,
  setSelectedModel,
}: {
  setAvailableModels: (value: ModelOption[] | ((previousModels: ModelOption[]) => ModelOption[])) => void;
  setSelectedModel: (value: string | undefined | ((previousModel: string | undefined) => string | undefined)) => void;
}): Promise<string | null> {
  try {
    const [settingsResponse, apiKeyResponse] = await Promise.all([
      fetch('/api/settings'),
      fetch('/api/settings/api-key'),
    ]);

    if (settingsResponse.ok && apiKeyResponse.ok) {
      const settings = await settingsResponse.json();
      const apiKeyInfo = await apiKeyResponse.json();
      const { api_base_url, api_provider, provider_type } = settings;
      const preferredModel = normalizeOptionalSelectValue(settings.llm_model);
      const storedApiKey = normalizeOptionalSelectValue(apiKeyInfo.api_key);

      if (hasUsableApiCredentials(storedApiKey, api_base_url)) {
        try {
          const resolvedApiKey = storedApiKey as string;
          const modelsResponse = await fetch(
            `/api/settings/models?api_key=${encodeURIComponent(resolvedApiKey)}&api_base_url=${encodeURIComponent(api_base_url)}&provider=${provider_type || api_provider}`
          );

          if (modelsResponse.ok) {
            const data = await modelsResponse.json();
            const normalizedModels = normalizeModelOptions(data.models);

            setAvailableModels((previousModels: ModelOption[]) => (
              areModelOptionsEqual(previousModels, normalizedModels) ? previousModels : normalizedModels
            ));

            setSelectedModel((previousModel: string | undefined) => (
              previousModel === preferredModel ? previousModel : preferredModel
            ));

            return preferredModel ?? null;
          }
        } catch (error) {
          console.error('Failed to load models list.', error);
        }
      }
    }
  } catch (error) {
    console.error('Failed to load model settings.', error);
  }

  return null;
}
