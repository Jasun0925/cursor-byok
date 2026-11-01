import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { PageContent } from "../../shell/layout/PageContent";
import { Button } from "../../shared/ui/Button";
import { FormField, TextInput } from "../../shared/ui/FormControls";
import { noteCliproxyListening } from "./cliproxyTab";
import styles from "./CliproxyPage.module.scss";

const DIRECTORY_KEY = "cursor-byok.cliproxy-directory";
const MANAGEMENT_URL = "http://localhost:8317/management.html";
const MAX_LOG_LINES = 400;

type CliproxyStatus = {
  owned: boolean;
  listening: boolean;
  logs: string[];
  seq: number;
  notice: string | null;
  notice_detail: string | null;
};

type LogEvent = {
  seq: number;
  line: string;
};

const emptyStatus: CliproxyStatus = {
  owned: false,
  listening: false,
  logs: [],
  seq: 0,
  notice: null,
  notice_detail: null,
};

function noticeText(notice: string | null, detail: string | null) {
  switch (notice) {
    case "empty-directory":
      return t("请先填写 CLIProxyAPI 所在目录");
    case "missing-directory":
      return t("目录不存在，没有启动");
    case "missing-binary":
      return t("目录里没有 cli-proxy-api，没有启动");
    case "external":
      return t("127.0.0.1:8317 已经在监听，不再启动第二份。这份不是本应用启动的，所以这里没有它的日志。停止会关掉这个端口上的 cli-proxy-api。");
    case "not-cliproxy":
      return t("8317 上的进程不是 cli-proxy-api，没有关闭。");
    case "spawn-failed":
      return t("启动失败：{detail}", { detail: detail ?? "" });
    case "stop-failed":
      return t("停止失败：{detail}", { detail: detail ?? "" });
    default:
      return null;
  }
}

export function CliproxyPage() {
  const desktop = isTauri();
  const [directory, setDirectory] = useState(() => localStorage.getItem(DIRECTORY_KEY) ?? "");
  const [status, setStatus] = useState<CliproxyStatus | null>(desktop ? null : emptyStatus);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const appliedSeq = useRef(0);
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    localStorage.setItem(DIRECTORY_KEY, directory);
  }, [directory]);

  useEffect(() => {
    const node = logRef.current;
    if (node) node.scrollTop = node.scrollHeight;
  }, [status?.logs]);

  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const apply = (next: CliproxyStatus) => {
      appliedSeq.current = next.seq;
      noteCliproxyListening(next.listening);
      setStatus(next);
    };
    const refresh = async () => {
      const next = await invoke<CliproxyStatus>("cliproxy_status");
      if (!disposed) apply(next);
    };
    const remember = (stop: () => void) => {
      if (disposed) stop();
      else unlisten.push(stop);
      return disposed;
    };
    void (async () => {
      const stopLog = await listen<LogEvent>("cliproxy-log", (event) => {
        const { seq, line } = event.payload;
        if (seq <= appliedSeq.current) return;
        if (seq !== appliedSeq.current + 1) {
          void refresh().catch((error: unknown) => {
            if (!disposed) setFailure(errorText(error));
          });
          return;
        }
        appliedSeq.current = seq;
        setStatus((current) => {
          const base = current ?? emptyStatus;
          return { ...base, seq, logs: [...base.logs, line].slice(-MAX_LOG_LINES) };
        });
      });
      if (remember(stopLog)) return;
      const stopStatus = await listen("cliproxy-status", () => {
        void refresh().catch((error: unknown) => {
          if (!disposed) setFailure(errorText(error));
        });
      });
      if (remember(stopStatus)) return;
      try {
        await refresh();
      } catch (error) {
        if (!disposed) setFailure(errorText(error));
      }
    })();
    return () => {
      disposed = true;
      for (const stop of unlisten) stop();
    };
  }, [desktop]);

  const owned = status?.owned ?? false;
  const external = Boolean(status && !status.owned && status.listening);
  const runLabel = owned ? t("运行中") : external ? t("已在运行") : t("未启动");
  const notice = noticeText(status?.notice ?? null, status?.notice_detail ?? null);

  async function run(action: "cliproxy_start" | "cliproxy_stop") {
    setBusy(true);
    setFailure(null);
    try {
      const next = await invoke<CliproxyStatus>(action, action === "cliproxy_start" ? { directory } : undefined);
      appliedSeq.current = next.seq;
      noteCliproxyListening(next.listening);
      setStatus(next);
    } catch (error) {
      setFailure(errorText(error));
    } finally {
      setBusy(false);
    }
  }

  async function openManagement() {
    setFailure(null);
    try {
      if (desktop) await openUrl(MANAGEMENT_URL);
      else window.open(MANAGEMENT_URL, "_blank", "noopener");
    } catch (error) {
      setFailure(errorText(error));
    }
  }

  async function pickDirectory() {
    setFailure(null);
    try {
      const picked = await invoke<string | null>("cliproxy_pick_directory", { current: directory });
      if (picked) setDirectory(picked);
    } catch (error) {
      setFailure(errorText(error));
    }
  }

  return <PageContent fixed title="CLIProxyAPI" sections={[{
    key: "cliproxy",
    estimatedHeight: 720,
    content: <div className={styles.page}>
      {!desktop && <p className={styles.notice}>{t("这个页面只在桌面应用里可用。")}</p>}
      <div className={styles.toolbar}>
        <FormField className={styles.directory} label={t("所在目录")} hint={t("选择这个目录后，点击启动会在该目录运行 cli-proxy-api。最小化到托盘时它会继续运行。关闭窗口或退出本应用时会一并停止它。账号和模型仍在 CLIProxyAPI 自己的配置里。")}>
          <TextInput value={directory} spellCheck={false} disabled={!desktop || busy} onChange={(event) => setDirectory(event.target.value)} />
        </FormField>
        <div className={styles.actions}>
          <Button disabled={!desktop || busy} onClick={() => void pickDirectory()}>{t("选择目录")}</Button>
          <Button variant="primary" disabled={!desktop || busy || status == null || owned || external} onClick={() => void run("cliproxy_start")}>{t("启动")}</Button>
          <Button disabled={!desktop || busy || !status?.listening} onClick={() => void run("cliproxy_stop")}>{t("停止")}</Button>
          <Button disabled={!desktop} onClick={() => void openManagement()}>{t("管理页面")}</Button>
        </div>
      </div>
      <p className={styles.status}>{runLabel}</p>
      {notice && <p className={styles.notice}>{notice}</p>}
      {failure && <p className={styles.notice}>{failure}</p>}
      <pre ref={logRef} className={styles.log} aria-label={t("日志")}>{status?.logs.length ? status.logs.join("\n") : t("还没有日志")}</pre>
    </div>,
  }]} />;
}

function errorText(error: unknown) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return t("无法读取 CLIProxyAPI 的运行状态");
}
