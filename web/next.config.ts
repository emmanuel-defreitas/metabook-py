import type { NextConfig } from "next";

// In production Vercel routes /api/* to the FastAPI service before Next.js sees
// the request (see ../vercel.json). Locally, proxy /api/* to `make dev`.
const devApi = process.env.METABOOK_API_URL ?? "http://127.0.0.1:8001";

const nextConfig: NextConfig = {
  cacheComponents: true,
  typedRoutes: true,
  partialPrefetching: true,
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
  async rewrites() {
    if (process.env.NODE_ENV !== "development" && !process.env.METABOOK_API_URL) return [];
    return [{ source: "/api/:path*", destination: `${devApi}/api/:path*` }];
  },
};

export default nextConfig;
