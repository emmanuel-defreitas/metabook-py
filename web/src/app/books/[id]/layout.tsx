import localFont from "next/font/local";
import "./book.css";

const ui = localFont({ src: "../../../../public/fonts/fontzz/halden.woff2", variable: "--font-book-ui", weight: "100 900", display: "swap" });
const content = localFont({ src: "../../../../public/fonts/fontzz/alder.woff2", variable: "--font-book-content", weight: "100 900", display: "swap" });

export default function BookLayout({ children }: { children: React.ReactNode }) {
  return <div className={`${ui.variable} ${content.variable} w-full`}>{children}</div>;
}
