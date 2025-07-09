import React from 'react';

interface ButtonProps {
  children: React.ReactNode;
  onClick?: () => void;
  variant?: 'primary' | 'secondary' | 'success' | 'error';
  size?: 'small' | 'medium' | 'large';
  disabled?: boolean;
  loading?: boolean;
  className?: string;
}

export default function Button({
  children,
  onClick,
  variant = 'primary',
  size = 'medium',
  disabled = false,
  loading = false,
  className = ''
}: ButtonProps) {
  const baseClasses = 'button';
  const variantClasses = {
    primary: 'button-primary',
    secondary: 'button-secondary',
    success: 'button-success',
    error: 'button-error'
  };
  const sizeClasses = {
    small: 'button-small',
    medium: 'button-medium',
    large: 'button-large'
  };

  const classes = [
    baseClasses,
    variantClasses[variant],
    sizeClasses[size],
    disabled ? 'button-disabled' : '',
    loading ? 'button-loading' : '',
    className
  ].filter(Boolean).join(' ');

  return (
    <button
      className={classes}
      onClick={onClick}
      disabled={disabled || loading}
    >
      {loading ? (
        <span className="button-spinner">⟳</span>
      ) : (
        children
      )}
    </button>
  );
}
