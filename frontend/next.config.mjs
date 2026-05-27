/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
