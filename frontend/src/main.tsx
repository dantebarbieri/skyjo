import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { WasmProvider } from '@/contexts/wasm-context';
import { AuthProvider } from '@/contexts/auth-context';
import App from './App';
import SimulatorRoute from '@/routes/simulator';
import PlayRoute from '@/routes/play';
import PlayOnlineRoute from '@/routes/play-online';
import RulesRoute from '@/routes/rules';
import StrategiesRoute from '@/routes/strategies';
import GeneticManageRoute from '@/routes/genetic-manage';
import LoginRoute from '@/routes/login';
import AdminRoute from '@/routes/admin';
import SettingsRoute from '@/routes/settings';
import SetupRoute from '@/routes/setup';
import LeaderboardRoute from '@/routes/leaderboard';
import GameDetailRoute from '@/routes/game-detail';
import './index.css';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <AuthProvider>
        <WasmProvider>
          <Routes>
            <Route element={<App />}>
              <Route index element={<Navigate to="/rules" replace />} />
              <Route path="simulator" element={<SimulatorRoute />} />
              <Route path="play" element={<PlayRoute />} />
              <Route path="play/online" element={<PlayOnlineRoute />} />
              <Route path="play/online/:roomCode" element={<PlayOnlineRoute />} />
              <Route path="rules" element={<RulesRoute />} />
              <Route path="rules/strategies" element={<StrategiesRoute />} />
              <Route path="rules/strategies/Genetic/manage" element={<GeneticManageRoute />} />
              <Route path="rules/strategies/:strategyName" element={<StrategiesRoute />} />
              <Route path="login" element={<LoginRoute />} />
              <Route path="setup" element={<SetupRoute />} />
              <Route path="admin" element={<AdminRoute />} />
              <Route path="settings" element={<SettingsRoute />} />
              <Route path="leaderboard" element={<LeaderboardRoute />} />
              <Route path="leaderboard/:gameId" element={<GameDetailRoute />} />
            </Route>
          </Routes>
        </WasmProvider>
      </AuthProvider>
    </BrowserRouter>
  </StrictMode>
);
