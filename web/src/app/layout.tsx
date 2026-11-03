import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import { SiteNav } from "@/components/shell/site-nav";
import { Providers } from "./providers";
import "./globals.css";

const sans = Geist({ variable: "--font-sans", subsets: ["latin"] });
const mono = Geist_Mono({ variable: "--font-geist-mono", subsets: ["latin"] });

export const metadata: Metadata = {
  title: { default: "Metabook", template: "%s · Metabook" },
  description:
    "Analyse the structure of books: chapters, paragraphs, sentences and words, without ever exposing the text.",
};

export const viewport: Viewport = {
  themeColor: [
    { media: "(prefers-color-scheme: light)", color: "#f9fafd" },
    { media: "(prefers-color-scheme: dark)", color: "#0f1217" },
  ],
};

export default function RootLayout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" suppressHydrationWarning className={`${sans.variable} ${mono.variable} h-full`}>
      <body className="grain flex min-h-dvh flex-col bg-background text-foreground">
        <Providers>
          <SiteNav />
          <main className="relative z-0 flex flex-1 flex-col">{children}</main>
        </Providers>
      </body>
    </html>
  );
}
