import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useState } from "react";
import controls from "../../shared/ui/Controls.module.scss";
import { Icon } from "../../shared/ui/Icon";
import { openInNewIcon } from "../../shared/ui/icons";
import { useMessage } from "../../shared/ui/message";
import { TooltipTrigger } from "../../shared/ui/TooltipTrigger";

export function OpenCursorButton() {
  const [opening, setOpening] = useState(false);
  const message = useMessage();
  if (!isTauri()) return null;

  const open = async () => {
    if (opening) return;
    setOpening(true);
    try {
      await openUrl("cursor://");
    } catch (cause) {
      message(t("无法打开 Cursor：{error}", { error: cause instanceof Error ? cause.message : String(cause) }));
    } finally {
      setOpening(false);
    }
  };

  return <TooltipTrigger label={t("打开 Cursor")}>
    <button type="button" className={controls.iconButton} aria-label={t("打开 Cursor")} aria-busy={opening} disabled={opening} onClick={() => void open()}>
      <Icon icon={openInNewIcon} size="1.1em" />
    </button>
  </TooltipTrigger>;
}
