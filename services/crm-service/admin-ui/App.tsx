import { Routes, Route, Navigate, NavLink, useParams } from 'react-router-dom';
import { DealsKanban } from './components/DealsKanban';
import { ContactTimeline } from './components/ContactTimeline';

function ContactTimelineWrapper() {
  const { contactId } = useParams<{ contactId: string }>();
  if (!contactId) return <div>联系人ID缺失</div>;
  return <ContactTimeline contactId={contactId} />;
}

export function CrmAdminApp() {
  return (
    <div className="min-h-screen bg-gray-50">
      <header className="bg-white shadow">
        <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex justify-between items-center py-4">
            <h1 className="text-2xl font-bold text-gray-900">CRM Admin</h1>
            <nav className="space-x-4">
              <NavLink to="/deals" className="text-blue-600 hover:text-blue-800">交易看板</NavLink>
              <NavLink to="/contacts" className="text-blue-600 hover:text-blue-800">联系人</NavLink>
            </nav>
          </div>
        </div>
      </header>

      <main className="max-w-7xl mx-auto py-6 sm:px-6 lg:px-8">
        <Routes>
          <Route path="/" element={<Navigate to="/deals" replace />} />
          <Route path="/deals" element={<DealsKanban />} />
          <Route path="/contacts/:contactId/timeline" element={<ContactTimelineWrapper />} />
          <Route path="/contacts" element={<div>联系人列表 (待实现)</div>} />
        </Routes>
      </main>
    </div>
  );
}
