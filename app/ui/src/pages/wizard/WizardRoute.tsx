import { useParams } from "react-router";
import { WizardPage } from "./WizardPage";

export function WizardRoute() {
  const { titleId } = useParams();
  return <WizardPage key={titleId ?? "new"} />;
}
