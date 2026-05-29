import { execSync } from 'node:child_process';

// Build-time provenance, inlined into the static export so the deployed site can
// show when it was built and link back to the exact source commit. Both are
// resolved here at `next build` time; the git lookup falls back gracefully when
// building outside a checkout (e.g. a tarball).
const buildTime = new Date().toISOString();
let commitSha = '';
try {
  commitSha = execSync('git rev-parse --short HEAD', { stdio: ['ignore', 'pipe', 'ignore'] })
    .toString()
    .trim();
} catch {
  commitSha = '';
}

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  env: {
    NEXT_PUBLIC_BUILD_TIME: buildTime,
    NEXT_PUBLIC_COMMIT_SHA: commitSha,
  },
  // This is a fully client-side dApp (no API routes, server actions, or SSR data
  // fetching), so `next build` emits a static site to `out/` that any static host
  // (nginx, S3/CloudFront, GitHub Pages, `npx serve out`) can serve. No Node
  // runtime needed. `NEXT_PUBLIC_*` env is inlined at build time.
  output: 'export',
  trailingSlash: true,
  webpack: (config) => {
    // Optional deps pulled transitively by wagmi/viem connectors that we don't
    // use (the last is a React-Native-only storage shim from @metamask/sdk).
    config.externals.push(
      'pino-pretty',
      'lokijs',
      'encoding',
      '@react-native-async-storage/async-storage',
    );
    return config;
  },
};

export default nextConfig;
