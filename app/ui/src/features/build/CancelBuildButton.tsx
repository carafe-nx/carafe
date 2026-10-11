import { useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "@/shared/api/commands";
import { Button } from "@/shared/ui/Button";
import { ConfirmDialog } from "@/shared/ui/ConfirmDialog";

type CancelBuildButtonProps = {
  title: string;
  rebuild: boolean;
};

export function CancelBuildButton({ title, rebuild }: CancelBuildButtonProps) {
  const { t } = useTranslation();
  const [asking, setAsking] = useState(false);

  return (
    <>
      <Button variant="danger" onClick={() => setAsking(true)}>
        {t("buildTask.cancel")}
      </Button>
      <ConfirmDialog
        open={asking}
        title={t("buildTask.cancelTitle")}
        text={rebuild ? t("buildTask.cancelTextRebuild", { title }) : t("buildTask.cancelText")}
        confirmLabel={t("buildTask.cancel")}
        cancelLabel={t("buildTask.keepBuilding")}
        onConfirm={() => {
          setAsking(false);
          void api.cancelBuild();
        }}
        onCancel={() => setAsking(false)}
      />
    </>
  );
}
