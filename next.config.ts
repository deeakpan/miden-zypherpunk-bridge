import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  webpack: (config, { isServer }) => {
    // Allow importing WASM files
    config.experiments = {
      ...config.experiments,
      asyncWebAssembly: true,
    };

    // Ensure WASM files from public directory are handled correctly
    config.resolve.fallback = {
      ...config.resolve.fallback,
      fs: false,
      path: false,
    };

    // Set target to support top-level await and async/await
    if (!isServer) {
      config.target = ['web', 'es2022'];
    }

    // Prevent WASM from loading during SSR
    if (isServer) {
      config.resolve.alias = {
        ...config.resolve.alias,
        '@demox-labs/miden-sdk': false,
      };
    }

    return config;
  },
  // Empty turbopack config to silence warning (we're using webpack for WASM support)
  turbopack: {},
};

export default nextConfig;
