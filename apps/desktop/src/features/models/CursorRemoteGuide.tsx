import { useState } from "react";
import { api } from "../../shared/api";
import { Modal } from "../../shared/ui/Modal";
import { useMessage } from "../../shared/ui/message";
import styles from "./CursorRemoteGuide.module.scss";

const remoteSettings = '{\n  "cursorAgentHost.remoteInferenceRoute": "always"\n}';

export function CursorRemoteGuide({ open, onClose }: { open: boolean; onClose: () => void }) {
  const message = useMessage();
  const [copying, setCopying] = useState(false);
  const copy = async () => {
    setCopying(true);
    try {
      await api.copyCursorText(remoteSettings);
      message(t("远程配置已复制"));
    } catch {
      message(t("无法复制，请手动选中并复制下方配置。"));
    } finally {
      setCopying(false);
    }
  };

  return <Modal open={open} title={t("Remote SSH 使用指南")} closeLabel={t("关闭")} onClose={onClose} onSubmit={() => void copy()} submitLabel={t("复制远程配置")} submitDisabled={copying}>
    <div className={styles.guide}>
      <p>{t("先在本机开启 Cursor 接管，并确认所选 BYOK 模型可用。本页的接管状态只表示本机配置，远程主机需要单独设置。")}</p>
      <ol>
        <li>{t("使用 Cursor 3.21.16 或更新版本，通过 Remote SSH 连接目标主机。")}</li>
        <li>{t("打开编辑器设置，选择“远程 [SSH: 主机]”，搜索 cursorAgentHost.remoteInferenceRoute，并将值设为 always。")}</li>
        <li>{t("也可以将下方配置项合入该主机的远程 settings.json，保留原有的其他配置。")}</li>
      </ol>
      <pre className={styles.config} tabIndex={0} aria-label={t("远程 settings.json 配置示例")}>{remoteSettings}</pre>
      <ol start={4}>
        <li>{t("执行 Developer: Reload Window，重新加载远程窗口。")}</li>
        <li>{t("新建 Agent 对话，明确选择带供应商标记的 BYOK 模型，并用远端临时文件验证读取和 Shell 执行。")}</li>
      </ol>
      <p>{t("always 让远程 Agent 的模型请求经过本机 Cursor，再交给本机 BYOK。文件和终端工具仍在远程工作区执行。")}</p>
      <p>{t("基础对话通过不代表所有远程功能都已通过；仓库索引和 WebFetch 的文件引用仍在单独验证。")}</p>
    </div>
  </Modal>;
}
