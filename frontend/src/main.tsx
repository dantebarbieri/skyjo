import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { WasmProvider } from '@/contexts/wasm-context';
import App from './App';
import SimulatorRoute from '@/routes/simulator';
import PlayRoute from '@/routes/play';
import RulesRoute from '@/routes/rules';
import './index.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <WasmProvider>
        <Routes>
          <Route element={<App />}>
            <Route index element={<Navigate to="/rules" replace />} />
            <Route path="simulator" element={<SimulatorRoute />} />
            <Route path="play" element={<PlayRoute />} />
            <Route path="rules" element={<RulesRoute />} />
          </Route>
        </Routes>
      </WasmProvider>
    </BrowserRouter>
  </StrictMode>
);
