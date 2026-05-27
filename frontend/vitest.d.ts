// Pulls the @testing-library/jest-dom matcher augmentation (toBeInTheDocument,
// toBeDisabled, …) into the TS program so `tsc --noEmit` typechecks the tests.
import '@testing-library/jest-dom/vitest';
