// SPDX-License-Identifier: LicenseRef-Proprietary-DornerIT

import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
