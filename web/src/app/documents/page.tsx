import type { Metadata } from "next";
import { DocumentView } from "@/components/documents/document-view";

export const metadata: Metadata = {
  title: "Upload",
  description: "Upload an EPUB, HTML, Markdown, PDF or plain-text document to explore its structure.",
};

export default function DocumentsPage() {
  return <DocumentView />;
}
