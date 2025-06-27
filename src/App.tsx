import "./App.css";
import { Settings } from "./components/settings/Settings";
import HandyTextLogo from "./components/icons/HandyTextLogo";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";

function App() {
  return (
    <div className="min-h-screen flex flex-col w-full">
      <div className="flex flex-col items-center pt-6 gap-8 px-4 flex-1">
        <HandyTextLogo width={300} />
        <AccessibilityPermissions />
        <Settings />
      </div>
      <Footer />
    </div>
  );
}

export default App;
