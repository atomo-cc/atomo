/**
 * Custom Deal Pipeline View Component
 * 
 * This component extends the default Atomo Admin UI with a custom
 * Kanban-style deal pipeline view.
 */

import React from 'react';
import { DealStage, Deal } from '../../schema';

interface DealPipelineViewProps {
  deals: Deal[];
  onDealUpdate: (dealId: string, updates: Partial<Deal>) => void;
}

const STAGE_COLUMNS = [
  { stage: DealStage.LEAD, title: '潜在客户', color: '#e3f2fd' },
  { stage: DealStage.QUALIFIED, title: '已确认', color: '#f3e5f5' },
  { stage: DealStage.PROPOSAL, title: '提案中', color: '#fff3e0' },
  { stage: DealStage.NEGOTIATION, title: '谈判中', color: '#fce4ec' },
  { stage: DealStage.WON, title: '已成交', color: '#e8f5e8' },
  { stage: DealStage.LOST, title: '已失败', color: '#ffebee' },
];

export function CustomDealPipelineView({ deals, onDealUpdate }: DealPipelineViewProps) {
  const dealsByStage = React.useMemo(() => {
    return STAGE_COLUMNS.reduce((acc, column) => {
      acc[column.stage] = deals.filter(deal => deal.stage === column.stage);
      return acc;
    }, {} as Record<DealStage, Deal[]>);
  }, [deals]);

  const handleDragEnd = (result: any) => {
    if (!result.destination) return;

    const { source, destination, draggableId } = result;
    const dealId = draggableId;
    const newStage = destination.droppableId as DealStage;

    if (source.droppableId !== destination.droppableId) {
      onDealUpdate(dealId, { stage: newStage });
    }
  };

  const getTotalValue = (stage: DealStage) => {
    return dealsByStage[stage]?.reduce((sum, deal) => sum + deal.value, 0) || 0;
  };

  return (
    <div className="deal-pipeline-view">
      <div className="pipeline-header">
        <h2>销售管道</h2>
        <div className="pipeline-stats">
          总价值: ¥{deals.reduce((sum, deal) => sum + deal.value, 0).toLocaleString()}
        </div>
      </div>

      <div className="pipeline-columns">
        {STAGE_COLUMNS.map(column => (
          <div 
            key={column.stage} 
            className="pipeline-column"
            style={{ backgroundColor: column.color }}
          >
            <div className="column-header">
              <h3>{column.title}</h3>
              <div className="column-stats">
                <span className="deal-count">{dealsByStage[column.stage]?.length || 0}</span>
                <span className="total-value">
                  ¥{getTotalValue(column.stage).toLocaleString()}
                </span>
              </div>
            </div>

            <div className="column-content">
              {dealsByStage[column.stage]?.map(deal => (
                <DealCard 
                  key={deal.id} 
                  deal={deal} 
                  onUpdate={(updates) => onDealUpdate(deal.id, updates)}
                />
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

interface DealCardProps {
  deal: Deal;
  onUpdate: (updates: Partial<Deal>) => void;
}

function DealCard({ deal, onUpdate }: DealCardProps) {
  const daysUntilClose = deal.expectedCloseDate 
    ? Math.ceil((new Date(deal.expectedCloseDate).getTime() - Date.now()) / (1000 * 60 * 60 * 24))
    : null;

  return (
    <div className="deal-card" draggable>
      <div className="deal-header">
        <h4 className="deal-title">{deal.title}</h4>
        <span className="deal-value">¥{deal.value.toLocaleString()}</span>
      </div>
      
      <div className="deal-meta">
        {deal.contactId && (
          <div className="deal-contact">
            联系人: {deal.contactId}
          </div>
        )}
        
        {daysUntilClose !== null && (
          <div className={`deal-timeline ${daysUntilClose < 0 ? 'overdue' : daysUntilClose < 7 ? 'urgent' : ''}`}>
            {daysUntilClose < 0 
              ? `逾期 ${Math.abs(daysUntilClose)} 天`
              : `${daysUntilClose} 天到期`
            }
          </div>
        )}
      </div>

      <div className="deal-actions">
        <button 
          onClick={() => onUpdate({ stage: DealStage.WON })}
          className="btn-win"
          disabled={deal.stage === DealStage.WON || deal.stage === DealStage.LOST}
        >
          成交
        </button>
        <button 
          onClick={() => onUpdate({ stage: DealStage.LOST })}
          className="btn-lose"
          disabled={deal.stage === DealStage.WON || deal.stage === DealStage.LOST}
        >
          失败
        </button>
      </div>
    </div>
  );
}
