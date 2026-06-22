import { useCallback, useState } from "react";
import { checkAppUpdate, openAppUpdate } from "../lib/api";
import type { AppUpdateInfo } from "../lib/types";
import { errorMessage } from "../lib/utils";

type UseAppUpdateOptions = {
  appVersion: string;
  setMessage: (message: string) => void;
};

export function useAppUpdate({ appVersion, setMessage }: UseAppUpdateOptions) {
  const [appUpdate, setAppUpdate] = useState<AppUpdateInfo | null>(null);
  const [checkingAppUpdate, setCheckingAppUpdate] = useState(false);
  const [openingAppUpdate, setOpeningAppUpdate] = useState(false);

  const refreshAppUpdate = useCallback(async () => {
    setCheckingAppUpdate(true);
    try {
      const result = await checkAppUpdate(appVersion);
      setAppUpdate(result);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setCheckingAppUpdate(false);
    }
  }, [appVersion, setMessage]);

  const openUpdatePage = useCallback(async () => {
    setOpeningAppUpdate(true);
    try {
      await openAppUpdate();
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setOpeningAppUpdate(false);
    }
  }, [setMessage]);

  return {
    appUpdate,
    checkingAppUpdate,
    openingAppUpdate,
    refreshAppUpdate,
    openUpdatePage
  };
}
