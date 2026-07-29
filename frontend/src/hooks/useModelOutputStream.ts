import { useCallback, useState } from 'react';

export const MODEL_OUTPUT_CHANNEL_LIMIT = 50_000;

type BoundedAppendResult = {
  value: string;
  truncated: boolean;
};

const appendBoundedTail = (current: string, chunk: string): BoundedAppendResult => {
  if (!chunk) {
    return { value: current, truncated: false };
  }

  const next = current + chunk;
  if (next.length <= MODEL_OUTPUT_CHANNEL_LIMIT) {
    return { value: next, truncated: false };
  }

  return {
    value: next.slice(-MODEL_OUTPUT_CHANNEL_LIMIT),
    truncated: true,
  };
};

export const useModelOutputStream = () => {
  const [reasoningContent, setReasoningContent] = useState('');
  const [generatedContent, setGeneratedContent] = useState('');
  const [reasoningTruncated, setReasoningTruncated] = useState(false);
  const [contentTruncated, setContentTruncated] = useState(false);

  const resetModelOutput = useCallback(() => {
    setReasoningContent('');
    setGeneratedContent('');
    setReasoningTruncated(false);
    setContentTruncated(false);
  }, []);

  const onReasoningChunk = useCallback((chunk: string) => {
    setReasoningContent((current) => {
      const next = appendBoundedTail(current, chunk);
      if (next.truncated) {
        setReasoningTruncated(true);
      }
      return next.value;
    });
  }, []);

  const onChunk = useCallback((chunk: string) => {
    setGeneratedContent((current) => {
      const next = appendBoundedTail(current, chunk);
      if (next.truncated) {
        setContentTruncated(true);
      }
      return next.value;
    });
  }, []);

  return {
    reasoningContent,
    generatedContent,
    reasoningTruncated,
    contentTruncated,
    resetModelOutput,
    onReasoningChunk,
    onChunk,
  };
};
