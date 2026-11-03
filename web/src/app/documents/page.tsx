import type { Metadata } from "next";
import { DocumentView } from "@/components/documents/document-view";

export const metadata: Metadata = {
  title: "Documents",
  description: "Outline an HTML, Markdown, PDF or plain-text document by heading level.",
};

export default function DocumentsPage() {
  return <DocumentView />;
}
