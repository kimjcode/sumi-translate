import { getCurrentWindow } from "@tauri-apps/api/window";
import GlanceCard from "./glance/GlanceCard";
import Workbench from "./workbench/Workbench";
import SettingsView from "./components/SettingsView";
import "./styles/tokens.css";
import "./App.css";

// 同一個前端 bundle 供多個視窗使用，以視窗 label 決定畫面。
const windowLabel = getCurrentWindow().label;

function App() {
  if (windowLabel === "glance") {
    document.documentElement.classList.add("glance-window");
    return <GlanceCard />;
  }
  if (windowLabel === "workbench") {
    document.documentElement.classList.add("workbench-window");
    return <Workbench />;
  }
  return <SettingsView />;
}

export default App;
