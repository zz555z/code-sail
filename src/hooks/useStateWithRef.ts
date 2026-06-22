import { useCallback, useRef, useState, type Dispatch, type SetStateAction } from "react";

/**
 * Custom hook that combines useState and useRef to provide both state and a ref
 * that always contains the current value. This is useful for accessing current
 * values in callbacks without adding them to dependency arrays.
 *
 * Returns [state, setState, stateRef] where stateRef.current always contains
 * the latest state value.
 */
export function useStateWithRef<T>(initialValue: T | (() => T)): [T, Dispatch<SetStateAction<T>>, React.MutableRefObject<T>] {
  const [state, setState] = useState<T>(initialValue);
  const ref = useRef<T>(state);
  ref.current = state;

  return [state, setState, ref];
}

/**
 * Custom hook that provides a stable callback that always calls the latest
 * version of the provided function. This avoids stale closure issues without
 * requiring the function to be in dependency arrays.
 */
export function useStableCallback<T extends (...args: never[]) => unknown>(callback: T): T {
  const ref = useRef<T>(callback);
  ref.current = callback;

  return useCallback((...args: Parameters<T>) => {
    return ref.current(...args);
  }, []) as T;
}
