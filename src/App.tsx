import { BrowserRouter, Routes, Route } from "react-router-dom";
import { DialogProvider } from "@/components/ui/dialog-provider";
import { AppLayout } from "@/components/layout/AppLayout";
import { Dashboard } from "@/pages/Dashboard";
import { Settings } from "@/pages/Settings";
import { Sources } from "@/pages/Sources";
import { Categories } from "@/pages/Categories";
import { Posts } from "@/pages/Posts";
import { PostEditor } from "@/pages/PostEditor";
import { History } from "@/pages/History";
import { Duplicates } from "@/pages/Duplicates";

function App() {
  return (
    <DialogProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<AppLayout />}>
          <Route path="/" element={<Dashboard />} />
          <Route path="/posts" element={<Posts />} />
          <Route path="/posts/:id" element={<PostEditor />} />
          <Route path="/sources" element={<Sources />} />
          <Route path="/categories" element={<Categories />} />
          <Route path="/history" element={<History />} />
          <Route path="/duplicates" element={<Duplicates />} />
          <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </DialogProvider>
  );
}

export default App;
