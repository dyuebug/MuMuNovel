import { useState, type ReactNode } from 'react';
import { Card, Row, Col, Typography, Image, Divider, Modal, Button, Space, Tag, theme } from 'antd';
import {
    HeartOutlined,
    CheckCircleOutlined,
    FileTextOutlined,
    RocketOutlined,
    MessageOutlined,
    // StarOutlined,
    WechatOutlined
} from '@ant-design/icons';
import { VERSION_INFO } from '../config/version';
import { designDisplayFont } from '../theme/themeConfig';

const { Title, Paragraph, Text } = Typography;

interface SponsorOption {
    amount: number | string;
    label: string;
    image: string;
    description: string;
}

interface SponsorBenefit {
    icon: ReactNode;
    title: string;
    description: string;
    price?: string;
}

const sponsorOptions: SponsorOption[] = [
    { amount: 5, label: '🌶️ 一包辣条', image: '/5.png', description: '¥5' },
    { amount: 10, label: '🍱 一顿拼好饭', image: '/10.png', description: '¥10' },
    { amount: 20, label: '☕ 一杯咖啡', image: '/20.png', description: '¥20' },
    { amount: 50, label: '🍖 一次烧烤', image: '/50.png', description: '¥50' },
    { amount: 99, label: '🍲 一顿海底捞', image: '/99.png', description: '¥99' },
];

const benefits: SponsorBenefit[] = [
    {
        icon: <WechatOutlined style={{ fontSize: '32px', color: 'var(--ant-color-primary)' }} />,
        title: '加入赞助群',
        description: '进入内部群，获取项目第一手更新消息',
        price: '（🌶️ 一包辣条）'
    },
    {
        icon: <FileTextOutlined style={{ fontSize: '32px', color: 'var(--ant-color-primary)' }} />,
        title: '优先需求响应',
        description: '您的功能需求和问题反馈将获得优先处理',
        price: '（🌶️ 一包辣条）'
    },
    {
        icon: <RocketOutlined style={{ fontSize: '32px', color: 'var(--ant-color-success)' }} />,
        title: 'Windows一键启动',
        description: '获取免安装一键启动包，开箱即可使用',
        price: '（🌶️ 一包辣条）'
    },
    {
        icon: <MessageOutlined style={{ fontSize: '32px', color: 'var(--ant-color-warning)' }} />,
        title: '专属技术支持',
        description: '获得远程协助和配置指导',
        price: '（☕ 一杯咖啡）'
    }
];

export default function Sponsor() {
    const [modalVisible, setModalVisible] = useState(false);
    const [selectedOption, setSelectedOption] = useState<SponsorOption | null>(null);
    const { token } = theme.useToken();
    const alphaColor = (color: string, alpha: number) =>
        `color-mix(in srgb, ${color} ${(alpha * 100).toFixed(0)}%, transparent)`;
    const editorialInk = '#f7f1e8';
    const pageBackground = `linear-gradient(180deg, ${alphaColor(token.colorPrimary, 0.06)} 0%, ${token.colorBgLayout} 30%, ${token.colorBgLayout} 100%)`;
    const heroBackground = `linear-gradient(135deg, #171411 0%, color-mix(in srgb, #171411 58%, ${token.colorPrimary} 42%) 100%)`;
    const quietPanelBackground = `linear-gradient(180deg, color-mix(in srgb, ${token.colorBgContainer} 94%, ${token.colorFillAlter} 6%) 0%, color-mix(in srgb, ${token.colorBgContainer} 86%, ${token.colorFillAlter} 14%) 100%)`;
    const panelBorder = alphaColor(token.colorPrimary, 0.12);
    const overviewStats = [
        { label: '支持档位', value: `${sponsorOptions.length} 档`, accent: token.colorPrimary },
        { label: '赞助权益', value: `${benefits.length} 项`, accent: token.colorSuccess },
        { label: '项目版本', value: VERSION_INFO.version, accent: token.colorInfo },
        { label: '联系入口', value: '微信 / QQ', accent: token.colorWarning },
    ];
    const handleCardClick = (option: SponsorOption) => {
        setSelectedOption(option);
        setModalVisible(true);
    };

    return (
        <div style={{
            minHeight: '100%',
            display: 'flex',
            flexDirection: 'column',
            overflow: 'hidden',
            background: pageBackground,
        }}>
            <div style={{
                flex: 1,
                overflowY: 'auto',
                overflowX: 'hidden',
                padding: '20px 16px 72px',
            }}>
                <div style={{
                    maxWidth: '1240px',
                    height: '100%',
                    margin: '0 auto',
                    width: '100%',
                    display: 'flex',
                    flexDirection: 'column',
                    minHeight: 'fit-content',
                    gap: 20,
                }}>
                    <Card
                        bordered={false}
                        style={{
                            background: heroBackground,
                            borderRadius: 28,
                            overflow: 'hidden',
                            boxShadow: `0 32px 68px -42px ${alphaColor(token.colorTextBase, 0.55)}`,
                        }}
                        styles={{ body: { padding: '24px' } }}
                    >
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 24, position: 'relative' }}>
                            <div
                                style={{
                                    position: 'absolute',
                                    inset: 0,
                                    background: 'radial-gradient(circle at top right, rgba(255,255,255,0.14), transparent 32%)',
                                    pointerEvents: 'none',
                                }}
                            />
                            <div style={{ position: 'relative', display: 'flex', flexDirection: 'column', gap: 12 }}>
                                <Text style={{ color: alphaColor(token.colorWhite, 0.68), letterSpacing: '0.14em', textTransform: 'uppercase' }}>
                                    Support the Studio
                                </Text>
                                <Title level={1} style={{ color: editorialInk, margin: 0, fontFamily: designDisplayFont, letterSpacing: '-0.03em', fontSize: 'clamp(28px, 5vw, 40px)' }}>
                                    赞助 {VERSION_INFO.projectName}
                                </Title>
                                <Paragraph style={{ color: alphaColor(token.colorWhite, 0.82), margin: 0, maxWidth: 760, fontSize: 'clamp(13px, 2vw, 15px)' }}>
                                    您的支持会直接转化成持续迭代、问题响应与创作体验优化，让 {VERSION_INFO.projectFullName} 继续向更完整的 AI 小说工作台演进。
                                </Paragraph>
                                <Space wrap>
                                    <Tag style={{ borderRadius: 999, paddingInline: 10, background: alphaColor(token.colorWhite, 0.12), color: editorialInk, border: 'none' }}>
                                        {VERSION_INFO.projectFullName}
                                    </Tag>
                                    <Tag style={{ borderRadius: 999, paddingInline: 10 }}>
                                        当前版本 {VERSION_INFO.version}
                                    </Tag>
                                </Space>
                            </div>

                            <Row gutter={[14, 14]} style={{ position: 'relative' }}>
                                {overviewStats.map((stat) => (
                                    <Col xs={24} sm={12} lg={6} key={stat.label}>
                                        <Card
                                            bordered={false}
                                            style={{
                                                height: '100%',
                                                borderRadius: 20,
                                                background: alphaColor(token.colorWhite, 0.08),
                                                boxShadow: `inset 0 1px 0 ${alphaColor(token.colorWhite, 0.12)}`,
                                            }}
                                            styles={{ body: { padding: 18 } }}
                                        >
                                            <Text style={{ color: alphaColor(token.colorWhite, 0.68), fontSize: 12 }}>{stat.label}</Text>
                                            <div style={{ marginTop: 10, display: 'flex', alignItems: 'center', gap: 10 }}>
                                                <span
                                                    style={{
                                                        width: 10,
                                                        height: 10,
                                                        borderRadius: 999,
                                                        background: stat.accent,
                                                        boxShadow: `0 0 0 6px ${alphaColor(stat.accent, 0.18)}`,
                                                        flexShrink: 0,
                                                    }}
                                                />
                                                <Text style={{ color: token.colorWhite, fontSize: 18, fontWeight: 600 }}>
                                                    {stat.value}
                                                </Text>
                                            </div>
                                        </Card>
                                    </Col>
                                ))}
                            </Row>
                        </div>
                    </Card>

                    <Card
                        bordered={false}
                        style={{
                            borderRadius: 24,
                            border: `1px solid ${panelBorder}`,
                            background: quietPanelBackground,
                            boxShadow: `0 24px 48px -42px ${alphaColor(token.colorTextBase, 0.45)}`,
                        }}
                        styles={{ body: { padding: '20px' } }}
                    >
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
                            <div>
                                <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                    Sponsor Perks
                                </Text>
                                <Title level={4} style={{ margin: '6px 0 0', fontFamily: designDisplayFont }}>
                                    <CheckCircleOutlined style={{ color: token.colorSuccess, marginRight: '8px' }} />
                                    赞助专属权益
                                </Title>
                            </div>

                            <Row
                                gutter={[{ xs: 8, sm: 12, md: 16 }, { xs: 8, sm: 12, md: 16 }]}
                                wrap={false}
                                style={{ overflowX: 'auto', paddingBottom: '4px' }}
                            >
                                {benefits.map((benefit, index) => (
                                    <Col key={index} flex="1" style={{ minWidth: '220px' }}>
                                        <Card
                                            hoverable
                                            style={{
                                                height: '100%',
                                                textAlign: 'center',
                                                borderRadius: '16px',
                                                boxShadow: `0 18px 34px -30px ${alphaColor(token.colorTextBase, 0.28)}`,
                                                border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                                            }}
                                            styles={{
                                                body: { padding: '20px 16px' }
                                            }}
                                        >
                                            <div style={{ marginBottom: '12px' }}>
                                                {benefit.icon}
                                            </div>
                                            <Title level={5} style={{ marginBottom: '8px', fontSize: 'clamp(14px, 2.5vw, 16px)', fontFamily: designDisplayFont }}>{benefit.title}</Title>
                                            <Paragraph style={{ color: token.colorTextSecondary, marginBottom: 0, fontSize: 'clamp(12px, 2vw, 13px)' }}>
                                                {benefit.description}
                                            </Paragraph>
                                            {benefit.price && (
                                                <Paragraph style={{ color: token.colorWarning, margin: '4px 0 0', fontSize: 'clamp(12px, 2vw, 13px)', fontWeight: 600 }}>
                                                    {benefit.price}
                                                </Paragraph>
                                            )}
                                        </Card>
                                    </Col>
                                ))}
                            </Row>

                            <Divider style={{ margin: 0 }} />

                            <div>
                                <Text style={{ fontSize: 12, letterSpacing: '0.12em', textTransform: 'uppercase', color: token.colorTextTertiary }}>
                                    Support Options
                                </Text>
                                <Title level={4} style={{ margin: '6px 0 18px', fontFamily: designDisplayFont }}>
                                    <HeartOutlined style={{ color: token.colorError, marginRight: '8px' }} />
                                    选择金额
                                </Title>

                                <Row gutter={[{ xs: 8, sm: 12, md: 16 }, { xs: 8, sm: 12, md: 16 }]} justify="center">
                                    {sponsorOptions.map((option, index) => (
                                        <Col xs={12} sm={8} md={6} lg={6} xl={4} key={index}>
                                            <Card
                                                hoverable
                                                onClick={() => handleCardClick(option)}
                                                style={{
                                                    textAlign: 'center',
                                                    borderRadius: '16px',
                                                    boxShadow: `0 18px 34px -30px ${alphaColor(token.colorTextBase, 0.28)}`,
                                                    cursor: 'pointer',
                                                    transition: 'all 0.3s',
                                                    border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`
                                                }}
                                                styles={{
                                                    body: { padding: '20px 12px' }
                                                }}
                                                onMouseEnter={(e) => {
                                                    e.currentTarget.style.transform = 'translateY(-8px)';
                                                    e.currentTarget.style.boxShadow = `0 18px 34px -20px ${alphaColor(token.colorPrimary, 0.32)}`;
                                                    e.currentTarget.style.borderColor = token.colorPrimary;
                                                }}
                                                onMouseLeave={(e) => {
                                                    e.currentTarget.style.transform = 'translateY(0)';
                                                    e.currentTarget.style.boxShadow = `0 18px 34px -30px ${alphaColor(token.colorTextBase, 0.28)}`;
                                                    e.currentTarget.style.borderColor = alphaColor(token.colorPrimary, 0.08);
                                                }}
                                            >
                                                <Title level={3} style={{
                                                    color: token.colorPrimary,
                                                    marginBottom: '4px',
                                                    fontSize: 'clamp(20px, 4vw, 28px)',
                                                    fontWeight: 'bold',
                                                    fontFamily: designDisplayFont,
                                                }}>
                                                    {option.description}
                                                </Title>
                                                <Text style={{ fontSize: 'clamp(12px, 2vw, 14px)', color: token.colorTextSecondary }}>
                                                    {option.label}
                                                </Text>
                                            </Card>
                                        </Col>
                                    ))}
                                </Row>
                            </div>

                            <div style={{
                                textAlign: 'center',
                                padding: '20px',
                                background: token.colorBgContainer,
                                borderRadius: '18px',
                                border: `1px solid ${alphaColor(token.colorPrimary, 0.08)}`,
                            }}>
                                <Title level={4} style={{ marginBottom: '12px', fontSize: 'clamp(16px, 3vw, 20px)', fontFamily: designDisplayFont }}>
                                    感谢您对 {VERSION_INFO.projectName} 的持续支持
                                </Title>
                                <Paragraph style={{ fontSize: 'clamp(12px, 2vw, 14px)', color: token.colorTextSecondary, marginBottom: 0 }}>
                                    每一次赞助都会转化为更稳定的版本发布、更快的问题响应，以及更细腻的创作体验打磨。
                                </Paragraph>
                            </div>
                        </div>
                    </Card>
                </div>
            </div>

            {/* 二维码弹窗 */}
            <Modal
                title={
                    <div style={{ textAlign: 'center' }}>
                        <Title level={3} style={{ marginBottom: '8px' }}>
                            {selectedOption?.description} {selectedOption?.label}
                        </Title>
                        <Text type="secondary">请使用微信扫码支付</Text>
                    </div>
                }
                open={modalVisible}
                onCancel={() => setModalVisible(false)}
                footer={[
                    <Button key="close" type="primary" onClick={() => setModalVisible(false)}>
                        关闭
                    </Button>
                ]}
                width={400}
                centered
            >
                <div style={{ textAlign: 'center', padding: '20px 0' }}>
                    <Image
                        src={selectedOption?.image}
                        alt={`${selectedOption?.description}赞助码`}
                        style={{
                            maxWidth: '280px',
                            borderRadius: '8px',
                            border: `1px solid ${token.colorBorderSecondary}`
                        }}
                        preview={false}
                    />
                    <Paragraph style={{ marginTop: '20px', color: token.colorTextSecondary }}>
                        扫描二维码完成支付
                    </Paragraph>
                    <Paragraph style={{ color: token.colorTextTertiary, fontSize: '12px' }}>
                        支付后可添加微信/QQ联系我们获取权益
                    </Paragraph>
                </div>
            </Modal>
        </div>
    );
}
