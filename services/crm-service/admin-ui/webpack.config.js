const path = require('path');
const HtmlWebpackPlugin = require('html-webpack-plugin');

module.exports = {
  mode: 'development',
  entry: './admin-ui/index.tsx',
  output: {
    path: path.resolve(__dirname, 'dist/admin'),
    filename: 'bundle.js',
    clean: true,
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js'],
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: 'ts-loader',
        exclude: /node_modules/,
      },
      {
        test: /\.css$/,
        use: ['style-loader', 'css-loader'],
      },
    ],
  },
  plugins: [
    new HtmlWebpackPlugin({
      template: './admin-ui/index.html',
      title: 'CRM Admin UI',
    }),
  ],
  devServer: {
    static: {
      directory: path.join(__dirname, 'dist/admin'),
    },
    compress: true,
    port: 3001,
    historyApiFallback: true,
    proxy: {
      '/api': 'http://localhost:3000',
      '/graphql': 'http://localhost:3000',
    },
  },
};
