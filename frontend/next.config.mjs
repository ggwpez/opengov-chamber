/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
