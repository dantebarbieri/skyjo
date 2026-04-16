/**
 * Regression test for issue #19: OnlinePlayBoard had an early return for
 * RoundOver/GameOver before a useEffect hook, violating React's rules of hooks
 * and causing a blank screen crash.
 *
 * This structural test ensures all hooks in OnlinePlayBoard are called before
 * any conditional returns, preventing the bug from being reintroduced.
 */
import { describe, it, expect } from 'vitest';
import source from '../play-online.tsx?raw';

describe('OnlinePlayBoard hooks ordering (Issue #19 regression)', () => {
  // Extract the OnlinePlayBoard function body
  const fnStart = source.indexOf('function OnlinePlayBoard(');
  expect(fnStart).toBeGreaterThan(-1);

  // Find the opening brace of the function body (skip the parameter/type block)
  let parenDepth = 0;
  let bodyOpenBrace = -1;
  for (let i = fnStart; i < source.length; i++) {
    if (source[i] === '(') parenDepth++;
    else if (source[i] === ')') {
      parenDepth--;
      if (parenDepth === 0) {
        for (let j = i + 1; j < source.length; j++) {
          if (source[j] === '{') {
            bodyOpenBrace = j;
            break;
          }
        }
        break;
      }
    }
  }
  expect(bodyOpenBrace).toBeGreaterThan(-1);

  // Find the matching closing brace
  let braceDepth = 1;
  let bodyEnd = -1;
  for (let i = bodyOpenBrace + 1; i < source.length; i++) {
    if (source[i] === '{') braceDepth++;
    else if (source[i] === '}') {
      braceDepth--;
      if (braceDepth === 0) {
        bodyEnd = i;
        break;
      }
    }
  }
  const body = source.slice(bodyOpenBrace, bodyEnd + 1);

  it('has no eslint-disable comments for rules-of-hooks', () => {
    expect(body).not.toContain('eslint-disable');
  });

  it('calls all hooks before any early return statements', () => {
    const lines = body.split('\n');

    let lastHookLine = -1;
    let firstEarlyReturnLine = -1;

    for (let i = 0; i < lines.length; i++) {
      const trimmed = lines[i].trim();

      // Skip comments
      if (trimmed.startsWith('//') || trimmed.startsWith('*')) continue;

      // Detect hook calls: useState, useEffect, useRef, useCallback, useMemo, or custom useXxx hooks
      if (/\buse[A-Z]\w*\s*\(/.test(trimmed)) {
        lastHookLine = i;
      }

      // Detect early returns that render RoundOver/GameOver sub-components
      if (/^\s*return\s*[(<]/.test(lines[i])) {
        const context = lines.slice(Math.max(0, i - 3), i + 3).join('\n');
        if (/OnlineRoundSummary|OnlineGameOver/.test(context)) {
          if (firstEarlyReturnLine === -1) {
            firstEarlyReturnLine = i;
          }
        }
      }
    }

    expect(lastHookLine).toBeGreaterThan(-1);
    expect(firstEarlyReturnLine).toBeGreaterThan(-1);
    expect(lastHookLine).toBeLessThan(firstEarlyReturnLine);
  });
});
