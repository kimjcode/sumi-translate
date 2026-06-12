import { getCurrentWindow } from "@tauri-apps/api/window";
import GlanceCard from "./glance/GlanceCard";
import SettingsView from "./components/SettingsView";
import "./styles/tokens.css";
import "./App.css";

// 同一個前端 bundle 供兩個視窗使用，以視窗 label 決定畫面。
const windowLabel = getCurrentWindow().label;

function App() {
  if (windowLabel === "glance") {
    document.documentElement.classList.add("glance-window");
    return <GlanceCard />;
  }
  return <SettingsView />;
}

export default App;
